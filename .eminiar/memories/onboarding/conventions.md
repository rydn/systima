---
name: onboarding/conventions
kind: onboarding
tags: []
related: [file:src/app.rs, file:src/ui/graph.rs, file:src/sampler.rs, file:src/fixtures.rs, file:src/input.rs]
created: 2026-10-18T15:14:07.805Z
updated: 2026-10-18T15:14:07.805Z
---

Code style and habits in systima:

- rustfmt defaults, clippy clean at `-D warnings` on all targets and features. Doc comment at the top of every module saying what it owns. Edition 2024 (let-else, `is_none_or`, `div_ceil` are used freely).
- **No per-frame allocation in hot paths.** Panels draw into `&mut Buffer` via `buf[(x,y)].set_symbol/set_style` and `buf.set_stringn`; formatted numbers go through `format::*` into a reused `String` (`out.clear(); write!`). Title spans (`Vec<Span>`) are the one accepted per-frame allocation. Adding a ratatui `Table`/`Chart` would break the allocation budget test — do not.
- **Plain data across threads.** `Snapshot` carries no `sysinfo` types. The process list is `Arc<[ProcInfo]>`; compare with `Arc::ptr_eq` to decide whether to rebuild rows.
- **Expensive refreshes are wall-clock periodic**, not per tick (`sampler::Cadence`). When adding a new sysinfo refresh, profile it with `SYSTIMA_PROFILE=1` and give it a period if it costs more than ~2 ms.
- CPU percentages are stored as u64 tenths (`pct_tenths`), bytes as u64, rates as bytes/s. Graph ceilings come from `app::Scale` (hysteresis) and `format::nice_ceiling`.
- Colours: `theme::Rgb` everywhere, converted via `Theme::color()`; never hard-code `Color::Rgb`.
- Keys are mapped only in `input::map`; `App::act` is the single reducer; `ui` reads `App` and writes back only layout feedback (`table_height`, `net_width`, `disk_width`, `offset`).
- Tests sit in a `#[cfg(test)] mod tests` at the bottom of each module; shared synthetic data lives in `src/fixtures.rs` (always compiled, `#[doc(hidden)]`) so benches can use it. Widget tests render into `Buffer::empty` and compare rows as strings.
- Commits: conventional prefixes (`feat:`, `perf:`, `docs:`, `chore:`), a body of bullets for anything non-trivial, author `Ryan Dick <noxinoobin@gmail.com>`, **no Claude/AI attribution trailers**, commit at each logical checkpoint. No PR flow yet; `main` only.
- Newcomer traps: sysinfo's first CPU reading is 0 (the sampler warms up with MINIMUM_CPU_UPDATE_INTERVAL); per-process CPU is summed over cores (App normalises by core count unless `e` toggles it); `Disks::refresh(true)` with storage is ~30 ms on macOS — use `DiskRefreshKind::nothing().with_io_usage()`; `Components::refresh` is ~100 ms; macOS reports most processes as `Run`.
