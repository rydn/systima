mod format;
mod history;
mod sampler;
mod snapshot;
mod theme;

fn main() {
    let h = sampler::host_info();
    println!(
        "systima {} on {} ({} cpus)",
        env!("CARGO_PKG_VERSION"),
        h.hostname,
        h.ncpu
    );
}
