//! Headless performance check: drive the real sampler, render every
//! snapshot into an in-memory backend and report frame and sampler cost.

use std::io;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::app::App;
use crate::format::bytes as systima_fmt_bytes;
use crate::sampler::Msg;
use crate::ui;

pub fn run(
    mut app: App,
    rx: &Receiver<Msg>,
    frames: usize,
    width: u16,
    height: u16,
    dump: bool,
) -> io::Result<()> {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("test backend is infallible");
    let mut draw_times: Vec<Duration> = Vec::with_capacity(frames);
    let mut sample_times: Vec<Duration> = Vec::with_capacity(frames);
    let started = Instant::now();
    eprintln!(
        "systima --bench: {frames} frames at {width}x{height}, interval {:?}",
        app.interval
    );
    while draw_times.len() < frames {
        let Ok(msg) = rx.recv() else { break };
        if let Msg::Snapshot(s) = &msg {
            sample_times.push(s.sample_cost);
        }
        if app.handle(msg) {
            let t = Instant::now();
            terminal
                .draw(|f| ui::draw(f, &mut app))
                .expect("test backend is infallible");
            draw_times.push(t.elapsed());
        }
    }
    let wall = started.elapsed();
    if dump {
        println!("{}", render_text(terminal.backend()));
    }
    report("frame render", &mut draw_times);
    report("sampler tick", &mut sample_times);
    if let Some(snap) = app.snap.as_deref() {
        let mut rss = String::new();
        systima_fmt_bytes(&mut rss, snap.self_rss);
        println!(
            "self usage:   cpu {:.2}% of one core   rss {rss}",
            snap.self_cpu
        );
    }
    let procs = app.procs.src.len();
    println!(
        "processes: {procs}   history: {} samples   wall: {:.1}s",
        app.hist.cpu.len(),
        wall.as_secs_f32()
    );
    Ok(())
}

fn report(name: &str, v: &mut [Duration]) {
    if v.is_empty() {
        println!("{name}: no samples");
        return;
    }
    v.sort_unstable();
    let n = v.len();
    let sum: Duration = v.iter().sum();
    let pct = |p: f64| v[((n as f64 - 1.0) * p).round() as usize];
    println!(
        "{name:<13} n={n:<4} min {:>8.3}ms  avg {:>8.3}ms  p50 {:>8.3}ms  p99 {:>8.3}ms  max {:>8.3}ms",
        ms(v[0]),
        ms(sum / n as u32),
        ms(pct(0.5)),
        ms(pct(0.99)),
        ms(v[n - 1]),
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The backend's buffer as plain text, one line per row.
pub fn render_text(backend: &TestBackend) -> String {
    let buf = backend.buffer();
    let mut out = String::with_capacity((buf.area.width as usize + 1) * buf.area.height as usize);
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
