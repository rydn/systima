---
name: onboarding/commands
kind: onboarding
tags: []
related: [file:Makefile, file:Cargo.toml, file:.github/workflows/ci.yml, file:src/bench.rs]
created: 2026-10-18T15:13:50.800Z
updated: 2026-10-18T15:13:50.800Z
---

Toolchain: Rust 1.85+ (edition 2024); developed on cargo/rustc 1.95, macOS arm64. No services, no env vars required.

- Build: `cargo build` / `cargo build --release` (release profile: lto fat, codegen-units 1, strip, panic unwind so ratatui's panic hook can restore the terminal).
- Run: `./target/release/systima` (or `make run`). Options: `-i/--interval 250ms|1s|2`, `-t/--theme cyberpunk|matrix`, `--no-truecolor`, `--bench N`, `--size WxH`, `--dump`, `-h`, `-V`.
- Tests: `cargo test` (unit tests in each module + `durations_parse` in main). One test (`sampler::tests::sampler_produces_plausible_snapshot`) touches the live system.
- Allocation budget: `cargo test --features alloc-count --test alloc_budget -- --nocapture` (counting global allocator; ≤150 allocs per frame, ≤4 per unchanged-list snapshot).
- Lint: `cargo fmt --all --check`; `cargo clippy --all-targets --all-features -- -D warnings`. CI treats warnings as errors (RUSTFLAGS=-D warnings).
- Everything CI runs: `make check`.
- Benchmarks: `cargo bench` / `make bench` (criterion, synthetic data, ~2 min). Real-system timing: `make perf` = `SYSTIMA_PROFILE=1 ./target/release/systima --bench 10 --interval 250ms --dump` (prints per-stage sampler ms to stderr, frame/sampler percentiles, systima's own CPU/RSS, and the last frame as text — the only way to "see" the UI from a non-tty session).
- Headless smoke: `./target/release/systima --bench 3 --interval 250ms --size 120x40 --dump`.
- Install: `cargo install --path .`.
- Release: not set up yet (no tags/crates.io publish); bump `version` in Cargo.toml and tag.
- Docs lookup: `em_docs {action:"lookup", query}` covers ratatui 0.30, sysinfo 0.39, crossterm 0.29 and the README.
