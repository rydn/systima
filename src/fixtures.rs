//! Synthetic data for tests and benchmarks (no live system access).
#![doc(hidden)]

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use crate::app::App;
use crate::sampler::Cmd;
use crate::snapshot::{DiskInfo, HostInfo, Iface, ProcInfo, ProcState, Snapshot};
use crate::theme::{Palette, Theme};

pub fn fake_proc(pid: u32, name: &str, cpu: f32, mem: u64) -> ProcInfo {
    ProcInfo {
        pid,
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
            Iface {
                name: "en0".into(),
                rx_bps: 1000,
                tx_bps: 500,
                rx_total: 0,
                tx_total: 0,
            },
            Iface {
                name: "en1".into(),
                rx_bps: 10,
                tx_bps: 5,
                rx_total: 0,
                tx_total: 0,
            },
        ],
        disk_read_bps: 4096,
        disk_write_bps: 2048,
        disks: vec![DiskInfo {
            mount: "/".into(),
            total: 1 << 40,
            available: 1 << 39,
        }],
        temps: vec![],
        procs: Arc::from(procs),
        procs_running: 1,
        self_cpu: 0.3,
        self_rss: 12 << 20,
        sample_cost: Duration::from_millis(1),
    })
}

pub fn test_app() -> (App, Receiver<Cmd>) {
    let (tx, rx) = channel();
    let host = HostInfo {
        hostname: "test".into(),
        os: "TestOS".into(),
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

/// An app pre-loaded with `n` processes and a full history, for benchmarks.
pub fn loaded_app(n: usize, history: usize) -> App {
    let (mut app, _rx) = test_app();
    std::mem::forget(_rx);
    let procs: Vec<ProcInfo> = (0..n)
        .map(|i| {
            let i = i as u32;
            let mut p = fake_proc(
                i + 1,
                &format!("proc-{i:04}"),
                (i % 97) as f32 * 1.3,
                (i as u64 % 61) << 20,
            );
            p.state = if i % 11 == 0 {
                ProcState::Run
            } else {
                ProcState::Sleep
            };
            p
        })
        .collect();
    for k in 0..history {
        let mut s = fake_snapshot(Vec::new());
        let k = k as u64;
        s.cpu_total = ((k * 7) % 100) as f32;
        s.mem_used = (4u64 << 30) + (((k * 131) % 1000) << 20);
        s.net[0].rx_bps = (k * 977) % 3_000_000;
        s.net[0].tx_bps = (k * 331) % 800_000;
        s.disk_read_bps = (k * 4099) % 50_000_000;
        s.disk_write_bps = (k * 1511) % 9_000_000;
        s.procs = app.procs.src.clone();
        app.handle(crate::sampler::Msg::Snapshot(s));
    }
    let mut s = fake_snapshot(procs);
    s.cpu_total = 42.0;
    app.handle(crate::sampler::Msg::Snapshot(s));
    app
}
