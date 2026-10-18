# systima

A retro-cyberpunk terminal system monitor written in Rust.

bmon-style scrolling history graphs for CPU, memory, network and disk I/O, plus an
htop-style task list. Neon on near-black, box-drawing chrome, block-character graphs
with a vertical gradient so peaks glow. Built to be efficient: the UI thread sleeps
on a channel and only redraws when something changed, histories live in fixed-size
ring buffers, and allocation budgets are enforced by tests.

```
 ▓▒░ SYSTIMA ░▒▓  host ▸ Darwin 26.5.1 arm64 ▸ up 22d 06:02:13 ▸ load 2.90 4.40 4.36 ▸ 12 cpu ▸ 52°C       cyberpunk ▸ 1.00s
╭╢ CPU ▸   9.2% ▸ 12 cores ▸ 4.5 GHz ╟─────────────────────────────────╮╭╢ MEM ▸ 15G / 24G ╟────────────────────────────────╮
│ 0 ███░░░░░  35%  6 ░░░░░░░░   0%  100%┤      ▂  ▄                  ││RAM ████████░░░░  64%      24G┤                    │
│ 1 ███░░░░░  36%  7 ░░░░░░░░   0%      │     ▄█▃▂█▅  ▁              ││    used  15G                 │                    │
│ 2 ██░░░░░░  20%  8 ░░░░░░░░   0%      │    ▆██████▇▅█▂ ▃           ││    avail 18G                 │    ▁▁▁▂▂▂▂▂▂▃▃▃▃▃▃▃│
│ 3 █░░░░░░░  12%  9 █░░░░░░░   8%   50%┤   ▂████████████▆▇▅▃▂▁      ││    free  8.5G                │▄▄▄▅████████████████│
│ 4 ░░░░░░░░   0% 10 ░░░░░░░░   0%      │  ▂██████████████████████▅▃▁││SWP ████░░░░░░░░  37%         │████████████████████│
│ 5 ░░░░░░░░   0% 11 ░░░░░░░░   0%    0%┤▁▇██████████████████████████││    swap  749M/2.0G        12G┤████████████████████│
╰─────────────────────────────────────────────────────────────────────╯╰───────────────────────────────────────────────────╯
╭╢ NET ▸ all (7 ifaces) ▸ Σ ▼47G ▲89G ╟──────────────────────────────╮╭╢ DISK ▸ / 377G/460G 81% ╟─────────────────────────╮
│ ▼ RX  2.1K/s  avg 1.2K/s  peak 2.8K/s                              ││ ▼ RD  0B/s  avg 0B/s  peak 0B/s                   │
│ 5.0K/s┤                       ▂                              ▆▁    ││ 1.0K/s┤                                           │
│ 2.5K/s┤            ▃▆▄▁      ▄█▇▃▁▂▅                 ▁▂▅▇▃▁ ▃██    ││ 512B/s┤                                           │
│   0B/s┤▁▂▃▂▁▁▂▄▆███████▅▃▂▃▅████████▇▅▄▃▂▂▃▄▆▇█████████████████    ││   0B/s┤                                           │
│ ▲ TX  85K/s  avg 22K/s  peak 85K/s                                 ││ ▲ WR  96K/s  avg 24K/s  peak 96K/s                │
│ 100K/s┤                                                        ▃   ││ 100K/s┤                                          ▇│
│  50K/s┤                 ▂▅▃                        ▁▄▆▂        █   ││  50K/s┤         ▂                  ▅             █│
│   0B/s┤▁▂▁▁▂▃▂▁▁▂▃▄▅▇████████▆▄▃▂▁▁▂▃▄▅▆▇▇▆▅▄▃▂▁▁▂▃▅▆▇██████████   ││   0B/s┤▁▂▁▁▁▂▃▅█▇▃▁▁▁▂▁▁▁▁▁▂▂▃▅█▆▃▂▁▁▁▂▁▁▁▁▁▂▃▆█│
╰────────────────────────────────────────────────────────────────────╯╰───────────────────────────────────────────────────╯
╭╢ TASKS ▸ 770 total ▸ 299 running ▸ sort CPU▾ ╟─────────────────────────────────────────────────────────────────────────╮
│    PID USER         CPU%   MEM%    RSS S    TIME NAME                                                                  │
│  28394 ryandick     2.5%   1.0%   267M R  14d15h Google Chrome Helper (Renderer)                                       │
│  10359 ryandick     0.9%   0.0%    13M R    0:00 systima                                                               │
│   1242 ryandick     0.8%   0.6%   162M R  22d05h Terminal                                                              │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────── 1-15/770 ─╯
 ? help  c m p n sort  / filter  x kill  [ ] iface  ␣ pause  + - rate  t theme  q quit          systima 0.9% 14M ▸ sample 6.7ms ▸ 770 tasks
```

## Install

Prebuilt binaries are attached to every push on `main` as the rolling
[`latest`](https://github.com/rydn/systima/releases/tag/latest) pre-release, and to
[versioned releases](https://github.com/rydn/systima/releases) for `v*` tags.
Targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`; each tarball has a `.sha256` next to it.

```sh
# macOS Apple Silicon
curl -L https://github.com/rydn/systima/releases/download/latest/systima-aarch64-apple-darwin.tar.gz | tar xz
./systima-aarch64-apple-darwin/systima
```

macOS Gatekeeper may block the unsigned binary on first run: `xattr -d com.apple.quarantine systima`.

From source:

```sh
cargo install --path .
# or
cargo build --release && ./target/release/systima
```

Requires Rust 1.85+ (edition 2024). Tested on macOS (Apple Silicon); the sampler is
`sysinfo`, so Linux works too (temperatures and disk I/O availability vary by platform).

## Usage

```
systima [OPTIONS]

  -i, --interval <DUR>   sampling interval: 250ms, 1s, 2 (seconds) [default: 1s]
  -t, --theme <NAME>     cyberpunk | matrix [default: cyberpunk]
      --no-truecolor     use the 256-colour palette (auto-detected from COLORTERM)
      --bench <N>        headless: render N frames off-screen and print timings
      --size WxH         off-screen terminal size for --bench [default: 200x60]
      --dump             with --bench: print the last rendered frame as text
```

### Keys

| Key | Action |
|---|---|
| `↑ ↓ j k`, `PgUp PgDn`, `g G` | move / page / top / bottom |
| `c m p n u T` | sort by CPU, MEM, PID, NAME, USER, TIME (again to flip) |
| `r` | reverse sort |
| `/` | filter by name, user or PID (`Enter` apply, `Esc` clear) |
| `x` or `F9` | kill selected: `y` SIGTERM, `K` SIGKILL, anything else cancels |
| `Tab` | cycle panel focus |
| `[ ]` | previous / next network interface (or all) |
| `space` | pause sampling |
| `+ -` | faster / slower sampling (250 ms … 10 s) |
| `e` | show process CPU as % of all cores (default) or % of one core |
| `t` | cycle theme |
| `? h F1` | help |
| `q Esc ^C` | quit |

The layout adapts: ≥ 30 rows shows CPU/MEM, NET/DISK and the task list; 18–29 rows
collapses the graphs into one row; smaller terminals show just the task list.

## Performance

The monitor must not be the load. Three design rules keep it cheap:

1. **No polling.** One `mpsc` channel carries both sampler snapshots and keyboard
   events (read by a blocking input thread). The UI thread `recv()`s, drains any
   burst, and draws once. Between events it is asleep.
2. **Expensive refreshes run on wall-clock periods, not per tick.** The full process
   scan runs every 2 s, disk capacity every 10 s and temperatures every 30 s whatever
   the sampling interval is. Only CPU, memory, network counters and disk I/O counters
   refresh each tick.
3. **Fixed memory.** Histories are 600-sample ring buffers allocated once. Process
   rows are re-formatted into reused strings only when the process list changes; the
   sort/filter order is an index vector recomputed only when inputs change. Graphs
   write block characters straight into the frame buffer.

Measured on an M-series MacBook Pro, macOS 26, ~770 processes (`make perf`, `cargo bench`,
`cargo test --features alloc-count`):

| Metric | Value |
|---|---|
| Full frame render, 200×60, 500 processes | ~195 µs |
| Full frame render, 80×24 | ~44 µs |
| Sort 2000 rows | ~61 µs |
| Apply snapshot, unchanged process list | ~180 ns, 0 allocations |
| Apply snapshot, new 2000-process list | ~250 µs |
| Heap allocations per frame | 87 (budget 150, enforced by test) |
| Sampler tick without process scan | 5–7 ms |
| Sampler tick with process scan (every 2 s) | 25–35 ms |
| CPU at 1 s interval | ~1% of one core |
| CPU at 250 ms interval | ~2.7% of one core |
| Resident memory | ~14 MB |

Most of the remaining cost is `sysinfo`'s per-process `proc_pidinfo` calls on macOS; the
temperature (SMC) read is ~100 ms, which is why it only happens twice a minute.

Set `SYSTIMA_PROFILE=1` with `--bench` to print per-stage sampler timings.

## Architecture

```
src/main.rs      CLI parsing, thread wiring, blocking event loop
src/sampler.rs   background collector owning all sysinfo handles; Cmd channel for kill/interval
src/snapshot.rs  plain-data Snapshot (no sysinfo types leak into the UI)
src/app.rs       UI state: histories, process cache, sort/filter/selection, modes, reducer
src/input.rs     key → Action mapping per mode
src/history.rs   Ring<T>: fixed-capacity ring buffer
src/theme.rs     palettes, gradient, 256-colour fallback
src/ui/graph.rs  bmon-style HistoryGraph widget and stacked dual panels
src/ui/*.rs      CPU, MEM, task table, layout and overlays
src/bench.rs     headless --bench mode
benches/         criterion benchmarks on synthetic data
tests/           allocation budget (feature `alloc-count`)
```

## macOS notes

- Without root, processes owned by other users report 0 CPU/MEM and 16-character names.
- The `S` column follows `sysinfo`'s status mapping; macOS reports many processes as `R`.
- Per-volume disk I/O counters come from IOKit; APFS volumes sharing one container may
  report duplicated or zero I/O. The DISK panel sums all listed volumes.
- Interfaces `lo0`, `utun*`, `awdl0`, `llw0`, `bridge*`, `anpi*` are hidden from the
  default "all" view.

## Development

```sh
make check   # fmt, clippy -D warnings, tests, allocation budget
make bench   # criterion
make perf    # real sampler + headless render with per-stage timings and a frame dump
```

MIT license.
