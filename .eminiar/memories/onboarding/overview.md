---
name: onboarding/overview
kind: onboarding
tags: []
related: [file:src/main.rs, file:src/sampler.rs, file:src/app.rs, file:src/ui/mod.rs, file:README.md]
created: 2026-10-18T15:13:22.490Z
updated: 2026-10-18T15:13:22.490Z
---

systima is a single-binary Rust terminal system monitor with a retro-cyberpunk look, inspired by bmon (scrolling history graphs with a scaled axis in a left gutter) and htop (sortable, filterable task list with kill). It is for one person watching their own machine (primarily macOS on Apple Silicon; Linux works through the same `sysinfo` calls). The whole point is "beautiful but efficient": the monitor must not itself be the load.

Runtime pieces:
- **Sampler thread** (`src/sampler.rs`): owns all `sysinfo` handles (System, Networks, Disks, Components, Users), ticks on an absolute deadline, builds a plain-data `Snapshot` (`src/snapshot.rs`) and sends it over a `sync_channel<Msg>`. Receives `Cmd::{Kill, SetInterval, Shutdown}` from the UI.
- **Input thread**: blocking `crossterm::event::read()` forwarded into the same channel.
- **UI thread** (`src/main.rs::run_loop` + `src/app.rs::App::handle`): blocks on `recv()`, drains bursts, redraws only when state changed. `App` holds ring-buffer histories, the formatted process cache, sort/filter/selection, modes (Normal/Filter/ConfirmKill/Help) and the key reducer.
- **Rendering** (`src/ui/`): header, CPU panel (per-core bars + history), MEM panel (gauges + history), NET and DISK stacked RX/TX RD/WR bmon graphs, TASKS table, footer with systima's own CPU/RSS, help and kill overlays. Themes `cyberpunk` and `matrix` with a 256-colour fallback.
- **Headless mode** (`src/bench.rs`, `systima --bench N [--dump]`): real sampler + off-screen render for timings and a text frame dump; criterion benches in `benches/`; allocation budget test in `tests/`.

Dependencies: ratatui 0.30, crossterm 0.29, sysinfo 0.39; criterion (dev). No async runtime, no clap.
