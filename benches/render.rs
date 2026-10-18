//! Frame-render and state-update benchmarks on synthetic data.
//!
//! `cargo bench` — numbers are recorded in the README. These do not touch
//! the live system; `systima --bench N` measures the real sampler.

use std::sync::Arc;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use systima::fixtures::{fake_proc, fake_snapshot, loaded_app};
use systima::input::{Action, SortKey};
use systima::sampler::Msg;
use systima::ui;

fn frames(c: &mut Criterion) {
    for (w, h) in [(200u16, 60u16), (120, 40), (80, 24)] {
        let mut app = loaded_app(500, 600);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui::draw(f, &mut app)).unwrap();

        // Pure render + diff with unchanged state (what a keypress costs).
        c.bench_function(&format!("frame_static_{w}x{h}"), |b| {
            b.iter(|| {
                term.draw(|f| ui::draw(f, &mut app)).unwrap();
            })
        });

        // A new snapshot every frame: graphs scroll, so most graph cells differ.
        let procs = app.procs.src.clone();
        let mut k = 0u64;
        c.bench_function(&format!("tick_and_frame_{w}x{h}"), |b| {
            b.iter(|| {
                k += 1;
                let mut s = fake_snapshot(Vec::new());
                s.cpu_total = ((k * 7) % 100) as f32;
                s.net[0].rx_bps = (k * 977) % 3_000_000;
                s.disk_write_bps = (k * 1511) % 9_000_000;
                s.procs = procs.clone();
                app.handle(Msg::Snapshot(s));
                term.draw(|f| ui::draw(f, &mut app)).unwrap();
            })
        });
    }
}

fn state(c: &mut Criterion) {
    // Sorting 2000 rows (toggling the key re-sorts every iteration).
    let mut app = loaded_app(2000, 10);
    c.bench_function("sort_2000_cpu_flip", |b| {
        b.iter(|| app.act(Action::Sort(SortKey::Cpu)))
    });
    c.bench_function("sort_2000_name", |b| {
        b.iter(|| {
            app.act(Action::Sort(SortKey::Name));
            app.act(Action::Sort(SortKey::Cpu));
        })
    });

    // Filtering: substring over 2000 names.
    c.bench_function("filter_2000", |b| {
        b.iter(|| {
            app.act(Action::FilterStart);
            app.act(Action::FilterChar('9'));
            app.act(Action::FilterClear);
        })
    });

    // Applying a snapshot that carries a new process list (full row rebuild).
    let procs: Vec<_> = (0..2000)
        .map(|i| fake_proc(i, "proc", i as f32, (i as u64) << 10))
        .collect();
    let arc: Arc<[_]> = Arc::from(procs);
    c.bench_function("snapshot_new_proclist_2000", |b| {
        b.iter_batched(
            || {
                let mut s = fake_snapshot(Vec::new());
                // Clone into a fresh Arc so the pointer differs and rows rebuild.
                s.procs = Arc::from(arc.to_vec());
                s
            },
            |s| app.handle(Msg::Snapshot(s)),
            BatchSize::SmallInput,
        )
    });

    // Applying a snapshot with the same list (histories only, no rebuild).
    let same = app.procs.src.clone();
    c.bench_function("snapshot_same_proclist", |b| {
        b.iter_batched(
            || {
                let mut s = fake_snapshot(Vec::new());
                s.procs = same.clone();
                s
            },
            |s| app.handle(Msg::Snapshot(s)),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, frames, state);
criterion_main!(benches);
