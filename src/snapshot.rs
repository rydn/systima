//! Plain-data view of the machine at one instant. Produced by the sampler
//! thread, consumed by the UI. Contains no `sysinfo` types so the UI never
//! depends on the collector.

use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcState {
    Run,
    Sleep,
    Idle,
    Stop,
    Zombie,
    Other,
}

impl ProcState {
    pub fn glyph(self) -> char {
        match self {
            ProcState::Run => 'R',
            ProcState::Sleep => 'S',
            ProcState::Idle => 'I',
            ProcState::Stop => 'T',
            ProcState::Zombie => 'Z',
            ProcState::Other => '?',
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub user: Arc<str>,
    /// Percent of one core, summed over cores (sysinfo semantics; may exceed 100).
    pub cpu: f32,
    /// Resident memory in bytes.
    pub mem: u64,
    pub state: ProcState,
    pub run_time: u64,
}

#[derive(Debug, Clone)]
pub struct Iface {
    pub name: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Temp {
    pub celsius: f32,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub cpu_total: f32,
    pub cores: Vec<f32>,
    pub freq_mhz: u64,

    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,

    pub load: [f64; 3],
    pub uptime: u64,

    pub net: Vec<Iface>,
    pub disk_read_bps: u64,
    pub disk_write_bps: u64,
    pub disks: Vec<DiskInfo>,
    pub temps: Vec<Temp>,

    pub procs: Arc<[ProcInfo]>,
    pub procs_running: usize,

    pub self_cpu: f32,
    pub self_rss: u64,
    /// How long the sampler spent producing this snapshot.
    pub sample_cost: Duration,
}

/// Static facts gathered once at start-up.
#[derive(Debug, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ncpu: usize,
}
