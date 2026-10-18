//! Background collector. Owns every `sysinfo` handle, wakes once per
//! interval (sleeping to an absolute deadline so it does not drift), and
//! ships an immutable [`Snapshot`] to the UI over a channel. Commands
//! (kill, interval change) flow the other way and are serviced immediately
//! because the sleep is a `recv_timeout` on the command channel.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sysinfo::{
    Components, CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, Pid,
    ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, RefreshKind, Signal, System, Uid,
    UpdateKind, Users,
};

use crate::snapshot::{DiskInfo, HostInfo, Iface, ProcInfo, ProcState, Snapshot, Temp};

/// Set `SYSTIMA_PROFILE=1` (with `--bench`) to print per-stage sampler timings to stderr.
fn profiling() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SYSTIMA_PROFILE").is_some())
}

macro_rules! stage {
    ($name:expr, $t:ident) => {
        if profiling() {
            eprintln!(
                "  {:<10} {:>8.2}ms",
                $name,
                $t.elapsed().as_secs_f64() * 1000.0
            );
            $t = Instant::now();
        }
    };
}

/// Everything the UI thread can receive.
pub enum Msg {
    Snapshot(Box<Snapshot>),
    Input(crossterm::event::Event),
    /// A one-line status message (e.g. the result of a kill).
    Status(String),
}

/// Commands the UI sends to the sampler.
pub enum Cmd {
    Kill { pid: u32, force: bool },
    SetInterval(Duration),
    Shutdown,
}

#[derive(Debug, Clone, Copy)]
pub struct SamplerConfig {
    pub interval: Duration,
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
        }
    }
}

/// Wall-clock periods for the expensive refreshes, independent of the
/// sampling interval: faster sampling must not multiply their cost.
pub const PROCS_PERIOD: Duration = Duration::from_millis(2000);
pub const TEMPS_PERIOD: Duration = Duration::from_secs(30);
pub const STORAGE_PERIOD: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
struct Cadence {
    procs: u64,
    temps: u64,
    storage: u64,
}

impl Cadence {
    fn for_interval(interval: Duration) -> Cadence {
        let every =
            |p: Duration| (p.as_millis() / interval.as_millis().max(1)).clamp(1, 120) as u64;
        Cadence {
            procs: every(PROCS_PERIOD),
            temps: every(TEMPS_PERIOD),
            storage: every(STORAGE_PERIOD),
        }
    }
}

pub const MIN_INTERVAL: Duration = sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;
pub const MAX_INTERVAL: Duration = Duration::from_secs(10);

/// Interfaces that only add noise on a desktop: loopback, VPN tunnels,
/// AirDrop/AWDL, bridges, link-local Wi‑Fi helpers.
pub fn is_noise_iface(name: &str) -> bool {
    name == "lo"
        || name == "lo0"
        || name == "ap1"
        || name == "docker0"
        || [
            "utun", "awdl", "llw", "bridge", "gif", "stf", "anpi", "veth", "vmnet", "XHC", "pktap",
        ]
        .iter()
        .any(|p| name.starts_with(p))
}

pub fn host_info() -> HostInfo {
    let sys =
        System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()));
    HostInfo {
        hostname: System::host_name().unwrap_or_else(|| "localhost".into()),
        os: format!(
            "{} {}",
            System::name().unwrap_or_else(|| "unknown".into()),
            System::os_version().unwrap_or_default()
        )
        .trim()
        .to_string(),
        arch: System::cpu_arch(),
        ncpu: sys.cpus().len().max(1),
    }
}

struct Sampler {
    cadence: Cadence,
    sys: System,
    networks: Networks,
    disks: Disks,
    components: Components,
    users: Users,
    user_names: HashMap<Uid, Arc<str>>,
    unknown_user: Arc<str>,
    prev_net: HashMap<String, (u64, u64)>,
    procs: Arc<[ProcInfo]>,
    procs_running: usize,
    temps: Vec<Temp>,
    last: Instant,
    tick: u64,
    own_pid: Option<Pid>,
}

const PROC_KIND: fn() -> ProcessRefreshKind = || {
    ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_user(UpdateKind::OnlyIfNotSet)
};

impl Sampler {
    fn new(cfg: SamplerConfig) -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency())
                .with_memory(MemoryRefreshKind::everything()),
        );
        // Warm-up: CPU usage (global and per-process) is a delta between two
        // refreshes, so take the first one now and let the first real tick be
        // the second. MINIMUM_CPU_UPDATE_INTERVAL is enforced by the caller.
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, PROC_KIND());
        let own_pid = sysinfo::get_current_pid().ok();
        Self {
            cadence: Cadence::for_interval(cfg.interval),
            sys,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            users: Users::new_with_refreshed_list(),
            user_names: HashMap::new(),
            unknown_user: Arc::from("?"),
            prev_net: HashMap::new(),
            procs: Arc::from(Vec::new()),
            procs_running: 0,
            temps: Vec::new(),
            last: Instant::now(),
            tick: 0,
            own_pid,
        }
    }

    fn user_name(&mut self, uid: Option<&Uid>) -> Arc<str> {
        let Some(uid) = uid else {
            return self.unknown_user.clone();
        };
        if let Some(n) = self.user_names.get(uid) {
            return n.clone();
        }
        let name: Arc<str> = match self.users.get_user_by_id(uid) {
            Some(u) => Arc::from(u.name()),
            None => Arc::from(format!("{}", **uid)),
        };
        self.user_names.insert(uid.clone(), name.clone());
        name
    }

    fn refresh_procs(&mut self) {
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, PROC_KIND());
        let mut out: Vec<ProcInfo> = Vec::with_capacity(self.sys.processes().len());
        let mut running = 0;
        // Collect (pid, uid) first to avoid borrowing `self` twice.
        let mut uids: Vec<(Pid, Option<Uid>)> = Vec::with_capacity(out.capacity());
        for (pid, p) in self.sys.processes() {
            uids.push((*pid, p.user_id().cloned()));
        }
        for (pid, uid) in uids {
            let user = self.user_name(uid.as_ref());
            let Some(p) = self.sys.process(pid) else {
                continue;
            };
            let state = match p.status() {
                ProcessStatus::Run => ProcState::Run,
                ProcessStatus::Sleep => ProcState::Sleep,
                ProcessStatus::Idle => ProcState::Idle,
                ProcessStatus::Stop => ProcState::Stop,
                ProcessStatus::Zombie => ProcState::Zombie,
                _ => ProcState::Other,
            };
            if state == ProcState::Run {
                running += 1;
            }
            out.push(ProcInfo {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                user,
                cpu: p.cpu_usage(),
                mem: p.memory(),
                state,
                run_time: p.run_time(),
            });
        }
        self.procs = Arc::from(out);
        self.procs_running = running;
    }

    #[allow(unused_assignments)]
    fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().max(1e-3);
        self.last = now;

        let mut t = Instant::now();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        stage!("cpu+mem", t);

        let procs_tick = self.tick % self.cadence.procs == 0;
        if procs_tick {
            self.refresh_procs();
        } else if let Some(pid) = self.own_pid {
            self.sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                false,
                PROC_KIND(),
            );
        }
        stage!("procs", t);
        if self.tick % self.cadence.temps == 0 {
            self.components.refresh(true);
            self.temps.clear();
            for c in self.components.iter() {
                if let Some(t) = c.temperature() {
                    if t.is_finite() && t > 0.0 {
                        self.temps.push(Temp { celsius: t });
                    }
                }
            }
        }

        stage!("temps", t);
        // Network: cumulative totals → bytes/s, guarding against counter resets.
        self.networks.refresh(true);
        let mut net: Vec<Iface> = Vec::with_capacity(self.networks.len());
        for (name, data) in self.networks.iter() {
            if is_noise_iface(name) {
                continue;
            }
            let (rx_total, tx_total) = (data.total_received(), data.total_transmitted());
            let (prx, ptx) = self
                .prev_net
                .get(name)
                .copied()
                .unwrap_or((rx_total, tx_total));
            let drx = if rx_total >= prx {
                rx_total - prx
            } else {
                rx_total
            };
            let dtx = if tx_total >= ptx {
                tx_total - ptx
            } else {
                tx_total
            };
            self.prev_net.insert(name.clone(), (rx_total, tx_total));
            net.push(Iface {
                name: name.clone(),
                rx_bps: (drx as f32 / dt) as u64,
                tx_bps: (dtx as f32 / dt) as u64,
                rx_total,
                tx_total,
            });
        }
        net.sort_by(|a, b| a.name.cmp(&b.name));

        stage!("net", t);
        // Disks: `usage()` is already a delta since the previous refresh.
        if self.tick % self.cadence.storage == 0 {
            self.disks.refresh_specifics(
                true,
                DiskRefreshKind::nothing().with_io_usage().with_storage(),
            );
        } else {
            self.disks
                .refresh_specifics(false, DiskRefreshKind::nothing().with_io_usage());
        }
        let (mut rd, mut wr) = (0u64, 0u64);
        let mut disks: Vec<DiskInfo> = Vec::new();
        for d in self.disks.list() {
            let u = d.usage();
            rd = rd.saturating_add(u.read_bytes);
            wr = wr.saturating_add(u.written_bytes);
            let mount = d.mount_point().to_string_lossy();
            if d.total_space() > 0
                && !mount.starts_with("/System/Volumes")
                && !mount.starts_with("/private/var")
            {
                disks.push(DiskInfo {
                    mount: mount.into_owned(),
                    total: d.total_space(),
                    available: d.available_space(),
                });
            }
        }
        disks.sort_by(|a, b| {
            a.mount
                .len()
                .cmp(&b.mount.len())
                .then(a.mount.cmp(&b.mount))
        });

        stage!("disks", t);
        let (self_cpu, self_rss) = self
            .own_pid
            .and_then(|p| self.sys.process(p))
            .map(|p| (p.cpu_usage(), p.memory()))
            .unwrap_or((0.0, 0));

        let load = System::load_average();
        self.tick += 1;

        Snapshot {
            cpu_total: self.sys.global_cpu_usage(),
            cores: self.sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
            freq_mhz: self.sys.cpus().first().map(|c| c.frequency()).unwrap_or(0),
            mem_total: self.sys.total_memory(),
            mem_used: self.sys.used_memory(),
            mem_available: self.sys.available_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            load: [load.one, load.five, load.fifteen],
            uptime: System::uptime(),
            net,
            disk_read_bps: (rd as f32 / dt) as u64,
            disk_write_bps: (wr as f32 / dt) as u64,
            disks,
            temps: self.temps.clone(),
            procs: self.procs.clone(),
            procs_running: self.procs_running,
            self_cpu,
            self_rss,
            sample_cost: Duration::ZERO,
        }
    }

    fn kill(&mut self, pid: u32, force: bool) -> String {
        let pid = Pid::from_u32(pid);
        let Some(p) = self.sys.process(pid) else {
            return format!("kill: pid {pid} not found");
        };
        let name = p.name().to_string_lossy().into_owned();
        let sig = if force { Signal::Kill } else { Signal::Term };
        match p.kill_with(sig) {
            Some(true) => format!("sent {sig:?} to {pid} ({name})"),
            Some(false) => format!("kill {pid} ({name}) failed: permission denied?"),
            None => "kill: signal not supported on this platform".to_string(),
        }
    }
}

pub fn spawn_sampler(
    cfg: SamplerConfig,
    tx: SyncSender<Msg>,
    cmd_rx: Receiver<Cmd>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("systima-sampler".into())
        .spawn(move || run_sampler(cfg, tx, cmd_rx))
        .expect("spawn sampler thread")
}

fn run_sampler(cfg: SamplerConfig, tx: SyncSender<Msg>, cmd_rx: Receiver<Cmd>) {
    let mut s = Sampler::new(cfg);
    let mut interval = cfg.interval.clamp(MIN_INTERVAL, MAX_INTERVAL);
    // First real sample after the warm-up delay.
    let mut next = Instant::now() + MIN_INTERVAL;
    loop {
        // Service commands until the deadline.
        loop {
            let now = Instant::now();
            if now >= next {
                break;
            }
            match cmd_rx.recv_timeout(next - now) {
                Ok(Cmd::Kill { pid, force }) => {
                    let msg = s.kill(pid, force);
                    if tx.send(Msg::Status(msg)).is_err() {
                        return;
                    }
                }
                Ok(Cmd::SetInterval(d)) => {
                    interval = d.clamp(MIN_INTERVAL, MAX_INTERVAL);
                    s.cadence = Cadence::for_interval(interval);
                    s.tick = 0;
                    next = Instant::now() + MIN_INTERVAL.max(interval / 4);
                }
                Ok(Cmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
        let t0 = Instant::now();
        let mut snap = s.sample();
        snap.sample_cost = t0.elapsed();
        if tx.send(Msg::Snapshot(Box::new(snap))).is_err() {
            return;
        }
        let now = Instant::now();
        next += interval;
        if next < now {
            // We fell behind (sleep/wake, heavy load): resynchronise.
            next = now + interval;
        }
    }
}

/// Blocking keyboard/resize reader. Exits when the receiver is gone.
pub fn spawn_input(tx: SyncSender<Msg>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("systima-input".into())
        .spawn(move || {
            while let Ok(ev) = crossterm::event::read() {
                if tx.send(Msg::Input(ev)).is_err() {
                    break;
                }
            }
        })
        .expect("spawn input thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_interfaces_filtered() {
        for n in ["lo0", "utun3", "awdl0", "llw0", "bridge0", "anpi1"] {
            assert!(is_noise_iface(n), "{n}");
        }
        for n in ["en0", "en5", "eth0", "wlan0"] {
            assert!(!is_noise_iface(n), "{n}");
        }
    }

    #[test]
    fn cadence_tracks_wall_clock_periods() {
        let c = Cadence::for_interval(Duration::from_millis(1000));
        assert_eq!((c.procs, c.temps, c.storage), (2, 30, 10));
        let c = Cadence::for_interval(Duration::from_millis(250));
        assert_eq!((c.procs, c.temps, c.storage), (8, 120, 40));
        let c = Cadence::for_interval(Duration::from_secs(5));
        assert_eq!((c.procs, c.temps, c.storage), (1, 6, 2));
    }

    #[test]
    fn sampler_produces_plausible_snapshot() {
        let mut s = Sampler::new(SamplerConfig::default());
        std::thread::sleep(MIN_INTERVAL);
        let snap = s.sample();
        assert!(snap.mem_total > 0);
        assert!(!snap.cores.is_empty());
        assert!(!snap.procs.is_empty());
        assert!(snap.procs.iter().any(|p| p.pid == std::process::id()));
        assert!(snap.sample_cost == Duration::ZERO);
    }
}
