//! UI-side state: metric histories, the process cache, selection, sort and
//! filter, modes, and the reducer that applies messages and key actions.

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crossterm::event::Event;

use crate::format::{self, nice_ceiling};
use crate::history::Ring;
use crate::input::{Action, Mode, SortKey, map};
use crate::sampler::{Cmd, MAX_INTERVAL, MIN_INTERVAL, Msg};
use crate::snapshot::{HostInfo, ProcInfo, Snapshot};
use crate::theme::{Palette, Theme};

pub const HISTORY: usize = 600;
const STATUS_TTL: Duration = Duration::from_secs(5);
const INTERVALS_MS: [u64; 6] = [250, 500, 1000, 2000, 5000, 10000];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Procs,
    Net,
    Disk,
}

impl Focus {
    fn next(self) -> Focus {
        match self {
            Focus::Procs => Focus::Net,
            Focus::Net => Focus::Disk,
            Focus::Disk => Focus::Procs,
        }
    }
}

/// Auto-scaling Y axis with hysteresis so graphs do not jitter.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    pub ceiling: u64,
    low_frames: u8,
    floor: u64,
}

impl Scale {
    pub const fn new(floor: u64) -> Scale {
        Scale {
            ceiling: floor,
            low_frames: 0,
            floor,
        }
    }

    pub fn update(&mut self, window_max: u64) {
        let want = nice_ceiling(window_max.max(self.floor));
        if want > self.ceiling {
            self.ceiling = want;
            self.low_frames = 0;
        } else if window_max < self.ceiling * 2 / 5 {
            self.low_frames = self.low_frames.saturating_add(1);
            if self.low_frames >= 5 {
                self.ceiling = want;
                self.low_frames = 0;
            }
        } else {
            self.low_frames = 0;
        }
    }
}

pub struct Histories {
    /// Total CPU in tenths of a percent.
    pub cpu: Ring<u64>,
    /// Used memory in bytes.
    pub mem: Ring<u64>,
    pub rx: Ring<u64>,
    pub tx: Ring<u64>,
    pub disk_r: Ring<u64>,
    pub disk_w: Ring<u64>,
}

impl Histories {
    fn new() -> Histories {
        Histories {
            cpu: Ring::new(HISTORY),
            mem: Ring::new(HISTORY),
            rx: Ring::new(HISTORY),
            tx: Ring::new(HISTORY),
            disk_r: Ring::new(HISTORY),
            disk_w: Ring::new(HISTORY),
        }
    }

    fn clear(&mut self) {
        self.cpu.clear();
        self.mem.clear();
        self.rx.clear();
        self.tx.clear();
        self.disk_r.clear();
        self.disk_w.clear();
    }
}

/// One process, with its numeric columns pre-formatted. Strings are reused
/// across rebuilds, so a steady-state rebuild allocates nothing.
#[derive(Debug, Default)]
pub struct ProcRow {
    /// Index into `ProcCache::src`.
    pub idx: usize,
    pub cpu_val: f32,
    pub mem_val: u64,
    pub pid: String,
    pub cpu: String,
    pub mem_pct: String,
    pub rss: String,
    pub time: String,
}

#[derive(Default)]
pub struct ProcCache {
    pub src: Arc<[ProcInfo]>,
    pub rows: Vec<ProcRow>,
    /// Filtered + sorted indexes into `rows`.
    pub order: Vec<usize>,
}

impl ProcCache {
    pub fn proc_of(&self, order_idx: usize) -> Option<(&ProcRow, &ProcInfo)> {
        let row = self.rows.get(*self.order.get(order_idx)?)?;
        Some((row, &self.src[row.idx]))
    }
}

pub struct App {
    pub host: HostInfo,
    pub theme: Theme,
    pub hist: Histories,
    pub snap: Option<Box<Snapshot>>,
    pub procs: ProcCache,

    pub sort: SortKey,
    pub sort_desc: bool,
    pub filter: String,
    pub mode: Mode,
    pub focus: Focus,
    pub selected: usize,
    pub offset: usize,
    /// Rows the table could show last frame; set by the UI.
    pub table_height: usize,
    /// Columns of graph the net/disk panels showed last frame; set by the UI.
    pub net_width: usize,
    pub disk_width: usize,

    pub scale_rx: Scale,
    pub scale_tx: Scale,
    pub scale_dr: Scale,
    pub scale_dw: Scale,

    /// `None` = all interfaces summed.
    pub iface: Option<String>,
    pub paused: bool,
    pub normalize_cpu: bool,
    pub interval: Duration,
    pub status: Option<(String, Instant)>,
    pub kill_target: Option<(u32, String)>,
    pub quit: bool,
    pub frames: u64,

    cmd_tx: Sender<Cmd>,
    scratch: String,
}

impl App {
    pub fn new(host: HostInfo, theme: Theme, interval: Duration, cmd_tx: Sender<Cmd>) -> App {
        App {
            host,
            theme,
            hist: Histories::new(),
            snap: None,
            procs: ProcCache::default(),
            sort: SortKey::Cpu,
            sort_desc: true,
            filter: String::new(),
            mode: Mode::Normal,
            focus: Focus::Procs,
            selected: 0,
            offset: 0,
            table_height: 20,
            net_width: 60,
            disk_width: 60,
            scale_rx: Scale::new(1024),
            scale_tx: Scale::new(1024),
            scale_dr: Scale::new(1024),
            scale_dw: Scale::new(1024),
            iface: None,
            paused: false,
            normalize_cpu: true,
            interval,
            status: None,
            kill_target: None,
            quit: false,
            frames: 0,
            cmd_tx,
            scratch: String::with_capacity(32),
        }
    }

    /// Apply one message. Returns `true` if the screen needs redrawing.
    pub fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Snapshot(s) => {
                if self.paused {
                    return false;
                }
                self.apply_snapshot(s);
                true
            }
            Msg::Status(s) => {
                self.set_status(s);
                true
            }
            Msg::Input(Event::Key(k)) => {
                let a = map(self.mode, k);
                self.act(a);
                a != Action::None
            }
            Msg::Input(Event::Resize(..)) => true,
            Msg::Input(_) => false,
        }
    }

    pub fn set_status(&mut self, s: String) {
        self.status = Some((s, Instant::now()));
    }

    pub fn status_text(&self) -> Option<&str> {
        match &self.status {
            Some((s, at)) if at.elapsed() < STATUS_TTL => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn ncpu(&self) -> usize {
        self.snap
            .as_ref()
            .map(|s| s.cores.len())
            .unwrap_or(self.host.ncpu)
            .max(1)
    }

    /// Selected network rates for the current snapshot: (rx, tx) bytes/s.
    pub fn net_rates(&self, snap: &Snapshot) -> (u64, u64) {
        match &self.iface {
            None => snap
                .net
                .iter()
                .fold((0, 0), |(r, t), i| (r + i.rx_bps, t + i.tx_bps)),
            Some(name) => snap
                .net
                .iter()
                .find(|i| &i.name == name)
                .map(|i| (i.rx_bps, i.tx_bps))
                .unwrap_or((0, 0)),
        }
    }

    fn apply_snapshot(&mut self, snap: Box<Snapshot>) {
        self.hist.cpu.push((snap.cpu_total.max(0.0) * 10.0) as u64);
        self.hist.mem.push(snap.mem_used);
        let (rx, tx) = self.net_rates(&snap);
        self.hist.rx.push(rx);
        self.hist.tx.push(tx);
        self.hist.disk_r.push(snap.disk_read_bps);
        self.hist.disk_w.push(snap.disk_write_bps);

        let nw = self.net_width.max(1);
        let dw = self.disk_width.max(1);
        self.scale_rx.update(self.hist.rx.max_last(nw).unwrap_or(0));
        self.scale_tx.update(self.hist.tx.max_last(nw).unwrap_or(0));
        self.scale_dr
            .update(self.hist.disk_r.max_last(dw).unwrap_or(0));
        self.scale_dw
            .update(self.hist.disk_w.max_last(dw).unwrap_or(0));

        let procs_changed = !Arc::ptr_eq(&self.procs.src, &snap.procs);
        let mem_total = snap.mem_total;
        if procs_changed {
            self.procs.src = snap.procs.clone();
        }
        self.snap = Some(snap);
        if procs_changed {
            self.rebuild_rows(mem_total);
            self.reorder();
        }
    }

    /// Re-format every row from `procs.src`, reusing the row strings.
    fn rebuild_rows(&mut self, mem_total: u64) {
        let ncpu = self.ncpu() as f32;
        let norm = if self.normalize_cpu { ncpu } else { 1.0 };
        let src = self.procs.src.clone();
        let rows = &mut self.procs.rows;
        rows.resize_with(src.len(), ProcRow::default);
        for (i, (p, row)) in src.iter().zip(rows.iter_mut()).enumerate() {
            row.idx = i;
            row.cpu_val = p.cpu / norm;
            row.mem_val = p.mem;
            row.pid.clear();
            let _ = write!(row.pid, "{}", p.pid);
            format::pct_tenths(&mut row.cpu, (row.cpu_val.max(0.0) * 10.0) as u64);
            let mem_pct_tenths = if mem_total > 0 {
                p.mem.saturating_mul(1000) / mem_total
            } else {
                0
            };
            format::pct_tenths(&mut row.mem_pct, mem_pct_tenths);
            format::bytes(&mut row.rss, p.mem);
            format::short_duration(&mut row.time, p.run_time);
        }
    }

    /// Rebuild `order` from the filter and sort settings, keeping the
    /// selected PID selected where possible.
    fn reorder(&mut self) {
        let selected_pid = self.procs.proc_of(self.selected).map(|(_, p)| p.pid);
        let src = &self.procs.src;
        let rows = &self.procs.rows;
        let order = &mut self.procs.order;
        order.clear();
        let filter = self.filter.trim();
        for (i, row) in rows.iter().enumerate() {
            if filter.is_empty() || matches_filter(&src[row.idx], filter) {
                order.push(i);
            }
        }
        let key = self.sort;
        let desc = self.sort_desc;
        order.sort_unstable_by(|&a, &b| {
            let (ra, rb) = (&rows[a], &rows[b]);
            let (pa, pb) = (&src[ra.idx], &src[rb.idx]);
            let o = match key {
                SortKey::Cpu => ra.cpu_val.total_cmp(&rb.cpu_val),
                SortKey::Mem => ra.mem_val.cmp(&rb.mem_val),
                SortKey::Pid => pa.pid.cmp(&pb.pid),
                SortKey::Name => ci_cmp(&pa.name, &pb.name),
                SortKey::User => ci_cmp(&pa.user, &pb.user),
                SortKey::Time => pa.run_time.cmp(&pb.run_time),
            };
            let o = if desc { o.reverse() } else { o };
            o.then(pa.pid.cmp(&pb.pid))
        });
        // Follow the previously selected process.
        if let Some(pid) = selected_pid {
            if let Some(pos) = order.iter().position(|&i| src[rows[i].idx].pid == pid) {
                self.selected = pos;
            }
        }
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        let n = self.procs.order.len();
        if n == 0 {
            self.selected = 0;
            self.offset = 0;
            return;
        }
        self.selected = self.selected.min(n - 1);
        let h = self.table_height.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + h {
            self.offset = self.selected + 1 - h;
        }
        self.offset = self.offset.min(n.saturating_sub(1));
    }

    fn move_selection(&mut self, delta: isize) {
        let n = self.procs.order.len() as isize;
        if n == 0 {
            return;
        }
        self.selected = (self.selected as isize + delta).clamp(0, n - 1) as usize;
        self.clamp_selection();
    }

    fn set_interval(&mut self, d: Duration) {
        let d = d.clamp(MIN_INTERVAL, MAX_INTERVAL);
        if d == self.interval {
            return;
        }
        self.interval = d;
        let _ = self.cmd_tx.send(Cmd::SetInterval(d));
        self.hist.clear();
        self.scratch.clear();
        let _ = write!(self.scratch, "sampling every {} ms", d.as_millis());
        let s = std::mem::take(&mut self.scratch);
        self.set_status(s.clone());
        self.scratch = s;
    }

    fn step_interval(&mut self, faster: bool) {
        let cur = self.interval.as_millis() as u64;
        let next = if faster {
            INTERVALS_MS.iter().rev().find(|&&m| m < cur).copied()
        } else {
            INTERVALS_MS.iter().find(|&&m| m > cur).copied()
        };
        if let Some(ms) = next {
            self.set_interval(Duration::from_millis(ms));
        }
    }

    fn cycle_iface(&mut self, forward: bool) {
        let Some(snap) = &self.snap else { return };
        let names: Vec<&str> = snap.net.iter().map(|i| i.name.as_str()).collect();
        if names.is_empty() {
            self.iface = None;
            return;
        }
        // Position 0 = all, 1..=n = names.
        let cur = match &self.iface {
            None => 0,
            Some(n) => names
                .iter()
                .position(|x| x == n)
                .map(|p| p + 1)
                .unwrap_or(0),
        };
        let len = names.len() + 1;
        let next = if forward {
            (cur + 1) % len
        } else {
            (cur + len - 1) % len
        };
        self.iface = if next == 0 {
            None
        } else {
            Some(names[next - 1].to_string())
        };
        self.hist.rx.clear();
        self.hist.tx.clear();
        self.scale_rx = Scale::new(1024);
        self.scale_tx = Scale::new(1024);
    }

    pub fn act(&mut self, a: Action) {
        match a {
            Action::None => {}
            Action::Quit => self.quit = true,
            Action::Up => self.move_selection(-1),
            Action::Down => self.move_selection(1),
            Action::PageUp => self.move_selection(-(self.table_height.max(1) as isize)),
            Action::PageDown => self.move_selection(self.table_height.max(1) as isize),
            Action::Top => self.move_selection(isize::MIN / 2),
            Action::Bottom => self.move_selection(isize::MAX / 2),
            Action::Sort(k) => {
                if self.sort == k {
                    self.sort_desc = !self.sort_desc;
                } else {
                    self.sort = k;
                    self.sort_desc = k.default_desc();
                }
                self.reorder();
            }
            Action::ReverseSort => {
                self.sort_desc = !self.sort_desc;
                self.reorder();
            }
            Action::FilterStart => {
                self.mode = Mode::Filter;
                self.focus = Focus::Procs;
            }
            Action::FilterChar(c) => {
                self.filter.push(c);
                self.reorder();
            }
            Action::FilterBackspace => {
                self.filter.pop();
                self.reorder();
            }
            Action::FilterApply => self.mode = Mode::Normal,
            Action::FilterClear => {
                self.filter.clear();
                self.mode = Mode::Normal;
                self.reorder();
            }
            Action::KillPrompt => {
                if let Some((_, p)) = self.procs.proc_of(self.selected) {
                    self.kill_target = Some((p.pid, p.name.clone()));
                    self.mode = Mode::ConfirmKill;
                }
            }
            Action::ConfirmKill { force } => {
                if let Some((pid, _)) = self.kill_target.take() {
                    let _ = self.cmd_tx.send(Cmd::Kill { pid, force });
                }
                self.mode = Mode::Normal;
            }
            Action::Cancel => {
                self.kill_target = None;
                self.mode = Mode::Normal;
            }
            Action::FocusNext => self.focus = self.focus.next(),
            Action::IfacePrev => self.cycle_iface(false),
            Action::IfaceNext => self.cycle_iface(true),
            Action::Pause => {
                self.paused = !self.paused;
                let s = if self.paused { "paused" } else { "resumed" };
                self.set_status(s.to_string());
            }
            Action::CycleTheme => {
                let p: Palette = self.theme.palette.next();
                self.theme = Theme::new(p, self.theme.truecolor);
                self.set_status(format!("theme: {}", p.name()));
            }
            Action::NormalizeCpu => {
                self.normalize_cpu = !self.normalize_cpu;
                let total = self.snap.as_ref().map(|s| s.mem_total).unwrap_or(0);
                self.rebuild_rows(total);
                self.reorder();
                let s = if self.normalize_cpu {
                    "process CPU: % of all cores"
                } else {
                    "process CPU: % of one core"
                };
                self.set_status(s.to_string());
            }
            Action::IntervalUp => self.step_interval(true),
            Action::IntervalDown => self.step_interval(false),
            Action::Help => self.mode = Mode::Help,
        }
    }
}

fn matches_filter(p: &ProcInfo, filter: &str) -> bool {
    if let Ok(pid) = filter.parse::<u32>() {
        if p.pid == pid {
            return true;
        }
    }
    ci_contains(&p.name, filter) || ci_contains(&p.user, filter)
}

fn ci_contains(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let n = needle.as_bytes();
    hay.as_bytes()
        .windows(n.len())
        .any(|w| w.iter().zip(n).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

fn ci_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::snapshot::ProcState;
    use std::sync::mpsc;

    pub fn fake_proc(pid: u32, name: &str, cpu: f32, mem: u64) -> ProcInfo {
        ProcInfo {
            pid,
            parent: None,
            name: name.to_string(),
            user: Arc::from("ryan"),
            cpu,
            mem,
            state: ProcState::Sleep,
            run_time: pid as u64 * 7,
        }
    }

    pub fn fake_snapshot(procs: Vec<ProcInfo>) -> Box<Snapshot> {
        Box::new(Snapshot {
            at: Instant::now(),
            dt: 1.0,
            cpu_total: 42.0,
            cores: vec![10.0, 20.0, 30.0, 40.0],
            freq_mhz: 3200,
            mem_total: 16 << 30,
            mem_used: 8 << 30,
            mem_available: 8 << 30,
            swap_total: 2 << 30,
            swap_used: 1 << 20,
            load: [1.0, 0.5, 0.25],
            uptime: 90_000,
            net: vec![
                crate::snapshot::Iface {
                    name: "en0".into(),
                    rx_bps: 1000,
                    tx_bps: 500,
                    rx_total: 0,
                    tx_total: 0,
                },
                crate::snapshot::Iface {
                    name: "en1".into(),
                    rx_bps: 10,
                    tx_bps: 5,
                    rx_total: 0,
                    tx_total: 0,
                },
            ],
            disk_read_bps: 4096,
            disk_write_bps: 2048,
            disks: vec![crate::snapshot::DiskInfo {
                mount: "/".into(),
                total: 1 << 40,
                available: 1 << 39,
            }],
            temps: vec![],
            procs: Arc::from(procs),
            procs_running: 1,
            threads_hint: 0,
            self_cpu: 0.3,
            self_rss: 12 << 20,
            sample_cost: Duration::from_millis(1),
        })
    }

    pub fn test_app() -> (App, mpsc::Receiver<Cmd>) {
        let (tx, rx) = mpsc::channel();
        let host = HostInfo {
            hostname: "test".into(),
            os: "TestOS".into(),
            kernel: "1.0".into(),
            arch: "arm64".into(),
            ncpu: 4,
        };
        (
            App::new(
                host,
                Theme::new(Palette::Cyberpunk, true),
                Duration::from_secs(1),
                tx,
            ),
            rx,
        )
    }

    fn names(app: &App) -> Vec<&str> {
        (0..app.procs.order.len())
            .map(|i| app.procs.proc_of(i).unwrap().1.name.as_str())
            .collect()
    }

    #[test]
    fn scale_hysteresis() {
        let mut s = Scale::new(1024);
        s.update(5000);
        assert_eq!(s.ceiling, 5 * 1024);
        // Dropping below 40% for fewer than 5 frames keeps the ceiling.
        for _ in 0..4 {
            s.update(100);
        }
        assert_eq!(s.ceiling, 5 * 1024);
        s.update(100);
        assert_eq!(s.ceiling, 1024);
        // Growth is immediate.
        s.update(3_000_000);
        assert_eq!(s.ceiling, 5 * 1024 * 1024);
    }

    #[test]
    fn snapshot_builds_sorted_rows_and_histories() {
        let (mut app, _rx) = test_app();
        app.handle(Msg::Snapshot(fake_snapshot(vec![
            fake_proc(1, "launchd", 1.0, 100),
            fake_proc(2, "cargo", 80.0, 300),
            fake_proc(3, "Finder", 20.0, 200),
        ])));
        assert_eq!(names(&app), vec!["cargo", "Finder", "launchd"]);
        assert_eq!(app.hist.cpu.last(), Some(420));
        assert_eq!(app.hist.rx.last(), Some(1010));
        // CPU normalised by 4 cores: 80 / 4 = 20.0%
        assert_eq!(app.procs.proc_of(0).unwrap().0.cpu, "20.0%");
        app.act(Action::NormalizeCpu);
        assert_eq!(app.procs.proc_of(0).unwrap().0.cpu, "80.0%");
    }

    #[test]
    fn sort_flip_filter_and_selection_follow() {
        let (mut app, _rx) = test_app();
        app.handle(Msg::Snapshot(fake_snapshot(vec![
            fake_proc(1, "launchd", 1.0, 100),
            fake_proc(2, "cargo", 80.0, 300),
            fake_proc(3, "Finder", 20.0, 200),
        ])));
        app.act(Action::Sort(SortKey::Name));
        assert_eq!(names(&app), vec!["cargo", "Finder", "launchd"]);
        app.act(Action::Sort(SortKey::Name));
        assert_eq!(names(&app), vec!["launchd", "Finder", "cargo"]);
        app.act(Action::Sort(SortKey::Pid));
        assert_eq!(names(&app), vec!["launchd", "cargo", "Finder"]);
        // Select Finder, then filter to it.
        app.act(Action::Bottom);
        assert_eq!(app.procs.proc_of(app.selected).unwrap().1.pid, 3);
        app.act(Action::FilterStart);
        for c in "fin".chars() {
            app.act(Action::FilterChar(c));
        }
        assert_eq!(names(&app), vec!["Finder"]);
        assert_eq!(app.selected, 0);
        app.act(Action::FilterClear);
        assert_eq!(app.procs.order.len(), 3);
        // Selection followed PID 3 back to its position.
        assert_eq!(app.procs.proc_of(app.selected).unwrap().1.pid, 3);
        // Filter by pid works too.
        app.filter = "2".into();
        app.reorder();
        assert_eq!(names(&app), vec!["cargo"]);
    }

    #[test]
    fn selection_clamps_when_list_shrinks() {
        let (mut app, _rx) = test_app();
        app.table_height = 2;
        let many: Vec<ProcInfo> = (1..=10).map(|i| fake_proc(i, "p", i as f32, 1)).collect();
        app.handle(Msg::Snapshot(fake_snapshot(many)));
        app.act(Action::Bottom);
        assert_eq!(app.selected, 9);
        assert_eq!(app.offset, 8);
        app.handle(Msg::Snapshot(fake_snapshot(vec![
            fake_proc(1, "p", 1.0, 1),
            fake_proc(2, "q", 2.0, 1),
        ])));
        assert_eq!(app.selected, 1);
        assert!(app.offset <= 1);
        app.handle(Msg::Snapshot(fake_snapshot(vec![])));
        assert_eq!(app.selected, 0);
        assert_eq!(app.offset, 0);
    }

    #[test]
    fn kill_flow_sends_command() {
        let (mut app, rx) = test_app();
        app.handle(Msg::Snapshot(fake_snapshot(vec![fake_proc(
            42, "victim", 1.0, 1,
        )])));
        app.act(Action::KillPrompt);
        assert_eq!(app.mode, Mode::ConfirmKill);
        app.act(Action::ConfirmKill { force: true });
        assert_eq!(app.mode, Mode::Normal);
        match rx.try_recv() {
            Ok(Cmd::Kill {
                pid: 42,
                force: true,
            }) => {}
            _ => panic!("expected kill command"),
        }
    }

    #[test]
    fn interval_steps_and_clears_history() {
        let (mut app, rx) = test_app();
        app.handle(Msg::Snapshot(fake_snapshot(vec![])));
        assert_eq!(app.hist.cpu.len(), 1);
        app.act(Action::IntervalDown);
        assert_eq!(app.interval, Duration::from_millis(2000));
        assert!(
            matches!(rx.try_recv(), Ok(Cmd::SetInterval(d)) if d == Duration::from_millis(2000))
        );
        assert_eq!(app.hist.cpu.len(), 0);
        app.act(Action::IntervalUp);
        app.act(Action::IntervalUp);
        app.act(Action::IntervalUp);
        app.act(Action::IntervalUp);
        assert_eq!(app.interval, Duration::from_millis(250));
    }

    #[test]
    fn iface_cycle_and_pause() {
        let (mut app, _rx) = test_app();
        app.handle(Msg::Snapshot(fake_snapshot(vec![])));
        app.act(Action::IfaceNext);
        assert_eq!(app.iface.as_deref(), Some("en0"));
        app.act(Action::IfaceNext);
        assert_eq!(app.iface.as_deref(), Some("en1"));
        app.act(Action::IfaceNext);
        assert_eq!(app.iface, None);
        app.act(Action::IfacePrev);
        assert_eq!(app.iface.as_deref(), Some("en1"));
        app.handle(Msg::Snapshot(fake_snapshot(vec![])));
        assert_eq!(app.hist.rx.last(), Some(10));
        app.act(Action::Pause);
        assert!(!app.handle(Msg::Snapshot(fake_snapshot(vec![]))));
        assert_eq!(app.hist.rx.len(), 1);
    }
}
