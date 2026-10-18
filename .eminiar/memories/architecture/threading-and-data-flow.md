---
name: architecture/threading-and-data-flow
kind: decision
tags: [architecture, performance]
related: [file:src/sampler.rs, file:src/main.rs, file:src/app.rs, file:src/snapshot.rs]
created: 2026-10-18T15:11:47.646Z
updated: 2026-10-18T15:11:47.646Z
---

systima runs three OS threads and no async runtime: (1) a sampler thread (src/sampler.rs) that owns every sysinfo handle, sleeps to an absolute deadline via `recv_timeout` on a command channel (so Kill / SetInterval commands are serviced immediately), and ships an immutable plain-data `Snapshot` (src/snapshot.rs, no sysinfo types) over a `sync_channel<Msg>`; (2) an input thread doing blocking `crossterm::event::read()` into the same channel; (3) the UI thread, which blocks on `recv()`, drains the channel with `try_recv` so bursts coalesce, and redraws once only when state changed. There is no fixed-FPS loop and no `event::poll` timeout, so the UI thread is fully idle between events. The process list travels as `Arc<[ProcInfo]>` and is only cloned (pointer) on ticks that did not rescan processes; the UI rebuilds its formatted rows only when the Arc pointer changes. Rejected: tokio (adds nothing to blocking syscalls), event::poll polling loop (wakes up needlessly).
