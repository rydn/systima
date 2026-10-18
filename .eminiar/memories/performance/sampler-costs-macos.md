---
name: performance/sampler-costs-macos
kind: fact
tags: [performance, macos, sysinfo]
related: [file:src/sampler.rs, file:tests/alloc_budget.rs, file:benches/render.rs, file:src/bench.rs]
created: 2026-10-18T15:11:57.220Z
updated: 2026-10-18T15:11:57.220Z
---

Measured sampler stage costs on an M-series MacBook Pro, macOS 26, ~770 processes (release build, SYSTIMA_PROFILE=1 ./target/release/systima --bench N): cpu+mem refresh ~0.1 ms; network refresh ~1.5 ms; disk refresh with DiskRefreshKind::everything() 21–35 ms per tick but only ~3 ms with `.with_io_usage()` only (storage/statfs is the expensive part, so capacity refreshes every 10 s); full process scan (ProcessRefreshKind::nothing().with_cpu().with_memory().with_user(OnlyIfNotSet)) 23–26 ms; Components (SMC temperature) refresh 85–106 ms. Hence the wall-clock cadences in sampler.rs: PROCS_PERIOD 2 s, STORAGE_PERIOD 10 s, TEMPS_PERIOD 30 s, derived from the interval via `Cadence::for_interval` so faster sampling does not multiply their cost. Resulting self usage: ~1% of one core at 1 s interval, ~2.7% at 250 ms, 14 MB RSS. Frame render (criterion, TestBackend 200x60, 500 procs) ~195 µs; 87 heap allocations per frame (budget 150 enforced in tests/alloc_budget.rs); applying a snapshot with an unchanged process list is ~180 ns and allocation-free.
