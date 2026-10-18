---
name: onboarding/structure
kind: onboarding
tags: []
related: [file:src/main.rs, file:src/sampler.rs, file:src/app.rs, file:src/ui/graph.rs, file:src/ui/procs.rs, file:src/fixtures.rs, file:benches/render.rs, file:tests/alloc_budget.rs, file:Makefile]
created: 2026-10-18T15:13:38.594Z
updated: 2026-10-18T15:13:38.594Z
---

Layout (lib + thin bin; everything is in the library so benches and tests drive real code):

- `src/lib.rs` — module list only.
- `src/main.rs` — hand-rolled CLI (`--interval`, `--theme`, `--no-truecolor`, `--bench N`, `--size WxH`, `--dump`), thread wiring, `run_loop` (blocking event loop). Entry point.
- `src/sampler.rs` — `Sampler` (sysinfo handles), `Cadence` (wall-clock periods PROCS 2 s / STORAGE 10 s / TEMPS 30 s derived from the interval), `run_sampler` loop, `spawn_input`, `Msg` and `Cmd` enums, `is_noise_iface`, `host_info`. Hotspot for performance work.
- `src/snapshot.rs` — `Snapshot`, `ProcInfo`, `Iface`, `DiskInfo`, `Temp`, `HostInfo`. Plain data, no sysinfo types.
- `src/app.rs` — `App` state and reducer (`handle(Msg)`, `act(Action)`), `Histories`, `Scale` (axis hysteresis), `ProcCache`/`ProcRow` (pre-formatted rows + index order). Hotspot for behaviour changes.
- `src/input.rs` — `Mode`, `SortKey`, `Action`, `map(mode, key)`, `HELP_LINES`.
- `src/history.rs` — `Ring<T>` fixed-capacity ring buffer.
- `src/format.rs` — humanisers writing into a reused `String`; `nice_ceiling` 1-2-5-1024 ladder.
- `src/theme.rs` — `Rgb`, `Palette`, `Theme` (styles, gradient, by_load, xterm-256 fallback, `env_truecolor`).
- `src/ui/mod.rs` — `draw` (responsive layout: ≥28 body rows two graph rows, ≥16 one row, else table only), `panel` block factory, header/footer, NET/DISK panels, help and kill overlays.
- `src/ui/graph.rs` — `HistoryGraph` widget, `render_dual` stacked panels, `Unit`, `GUTTER`.
- `src/ui/cpu.rs`, `src/ui/mem.rs`, `src/ui/procs.rs` — the other panels.
- `src/bench.rs` — headless `--bench` runner and `render_text`.
- `src/fixtures.rs` — synthetic `fake_proc`, `fake_snapshot`, `test_app`, `loaded_app` for tests and benches.
- `benches/render.rs` — criterion; `tests/alloc_budget.rs` — allocation budget behind feature `alloc-count`.
- `Makefile` (check/test/bench/perf/run), `.github/workflows/ci.yml` (fmt, clippy -D warnings, tests, alloc budget, release build, headless smoke run on ubuntu + macos).
- `.eminiar/memories/` — decisions, facts and these onboarding notes; `.eminiar/docs.json` — docs manifest.
