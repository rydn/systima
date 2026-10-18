//! Allocation budget: a steady-state frame must stay under a fixed number
//! of heap allocations so regressions show up as test failures, not just
//! slower benchmarks.
//!
//! Run with `cargo test --features alloc-count --test alloc_budget`.
#![cfg(feature = "alloc-count")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use systima::fixtures::{fake_snapshot, loaded_app};
use systima::sampler::Msg;
use systima::ui;

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Heap allocations allowed per rendered frame at 200x60 with 500 processes.
/// Measured at 87 on 2026-10-18; the budget leaves headroom for ratatui's own
/// per-frame work (Line/Span vectors for titles, header, footer).
const FRAME_BUDGET: usize = 150;
/// Allocations allowed for applying a snapshot whose process list is unchanged.
const SNAPSHOT_BUDGET: usize = 4;

#[test]
fn frame_allocations_within_budget() {
    let mut app = loaded_app(500, 600);
    let mut term = Terminal::new(TestBackend::new(200, 60)).unwrap();
    term.draw(|f| ui::draw(f, &mut app)).unwrap();

    const N: usize = 20;
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..N {
        term.draw(|f| ui::draw(f, &mut app)).unwrap();
    }
    let per_frame = (ALLOCS.load(Ordering::Relaxed) - before) / N;
    println!("allocations per frame: {per_frame}");
    assert!(
        per_frame <= FRAME_BUDGET,
        "frame allocated {per_frame} times (budget {FRAME_BUDGET})"
    );
}

#[test]
fn same_proclist_snapshot_allocations_within_budget() {
    let mut app = loaded_app(500, 600);
    let procs = app.procs.src.clone();
    let mut snaps = Vec::new();
    for _ in 0..10 {
        let mut s = fake_snapshot(Vec::new());
        s.procs = procs.clone();
        snaps.push(s);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    for s in snaps {
        app.handle(Msg::Snapshot(s));
    }
    let per = (ALLOCS.load(Ordering::Relaxed) - before) / 10;
    println!("allocations per same-list snapshot: {per}");
    assert!(
        per <= SNAPSHOT_BUDGET,
        "snapshot apply allocated {per} times (budget {SNAPSHOT_BUDGET})"
    );
}
