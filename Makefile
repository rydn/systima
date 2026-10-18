.PHONY: check test bench perf run release

# Everything CI runs.
check:
	cargo fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test
	cargo test --features alloc-count --test alloc_budget -- --nocapture

test:
	cargo test

# Synthetic benchmarks (criterion): frame render, sort, filter, snapshot apply.
bench:
	cargo bench

# Real sampler + off-screen render; prints frame and sampler timings and a frame dump.
perf: release
	SYSTIMA_PROFILE=1 ./target/release/systima --bench 10 --interval 250ms --dump

release:
	cargo build --release

run: release
	./target/release/systima
