//! systima — a retro-cyberpunk terminal system monitor.

use std::io;
use std::sync::mpsc::{Receiver, channel, sync_channel};
use std::time::Duration;

use ratatui::DefaultTerminal;

use systima::app::App;
use systima::sampler::{self, Cmd, Msg, SamplerConfig};
use systima::theme::{self, Palette, Theme};
use systima::{bench, ui};

struct Args {
    interval: Duration,
    palette: Palette,
    truecolor: bool,
    bench: Option<usize>,
    size: (u16, u16),
    dump: bool,
}

const USAGE: &str = "\
systima — retro-cyberpunk terminal system monitor

USAGE: systima [OPTIONS]

OPTIONS:
  -i, --interval <DUR>   sampling interval: 250ms, 1s, 2 (seconds) [default: 1s]
  -t, --theme <NAME>     cyberpunk | matrix [default: cyberpunk]
      --no-truecolor     use the 256-colour palette (auto-detected from COLORTERM)
      --bench <N>        headless: render N frames off-screen and print timings
      --size WxH         off-screen terminal size for --bench [default: 200x60]
      --dump             with --bench: print the last rendered frame as text
  -h, --help             show this help
  -V, --version          show version
";

fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        return ms.trim().parse::<u64>().ok().map(Duration::from_millis);
    }
    if let Some(sec) = s.strip_suffix('s') {
        return sec.trim().parse::<f64>().ok().map(Duration::from_secs_f64);
    }
    let v: f64 = s.parse().ok()?;
    // Bare numbers: ≥ 50 means milliseconds, otherwise seconds.
    Some(if v >= 50.0 {
        Duration::from_millis(v as u64)
    } else {
        Duration::from_secs_f64(v)
    })
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        interval: Duration::from_secs(1),
        palette: Palette::Cyberpunk,
        truecolor: theme::env_truecolor(),
        bench: None,
        size: (200, 60),
        dump: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("systima {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-i" | "--interval" => {
                let v = value("--interval")?;
                args.interval = parse_duration(&v).ok_or_else(|| format!("bad interval: {v}"))?;
            }
            "-t" | "--theme" => {
                let v = value("--theme")?;
                args.palette =
                    Palette::from_name(&v).ok_or_else(|| format!("unknown theme: {v}"))?;
            }
            "--dump" => args.dump = true,
            "--no-truecolor" => args.truecolor = false,
            "--truecolor" => args.truecolor = true,
            "--bench" => {
                let v = value("--bench")?;
                args.bench = Some(v.parse().map_err(|_| format!("bad frame count: {v}"))?);
            }
            "--size" => {
                let v = value("--size")?;
                let (w, h) = v.split_once('x').ok_or_else(|| format!("bad size: {v}"))?;
                args.size = (
                    w.parse().map_err(|_| format!("bad size: {v}"))?,
                    h.parse().map_err(|_| format!("bad size: {v}"))?,
                );
            }
            other => return Err(format!("unknown option: {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn main() -> io::Result<()> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("systima: {e}");
            std::process::exit(2);
        }
    };

    let host = sampler::host_info();
    let (tx, rx) = sync_channel::<Msg>(16);
    let (cmd_tx, cmd_rx) = channel::<Cmd>();
    let interval = args
        .interval
        .clamp(sampler::MIN_INTERVAL, sampler::MAX_INTERVAL);
    let app = App::new(
        host,
        Theme::new(args.palette, args.truecolor),
        interval,
        cmd_tx.clone(),
    );
    let sampler = sampler::spawn_sampler(SamplerConfig { interval }, tx.clone(), cmd_rx);

    let result = if let Some(n) = args.bench {
        bench::run(app, &rx, n, args.size.0, args.size.1, args.dump)
    } else {
        let mut app = app;
        ratatui::run(|terminal| {
            sampler::spawn_input(tx.clone());
            run_loop(terminal, &mut app, &rx)
        })
    };

    let _ = cmd_tx.send(Cmd::Shutdown);
    drop(rx);
    let _ = sampler.join();
    result
}

/// Block on the channel; coalesce bursts into one draw; redraw only when
/// state changed. The thread is fully idle between events.
fn run_loop(terminal: &mut DefaultTerminal, app: &mut App, rx: &Receiver<Msg>) -> io::Result<()> {
    terminal.draw(|f| ui::draw(f, app))?;
    while let Ok(first) = rx.recv() {
        let mut dirty = app.handle(first);
        while let Ok(more) = rx.try_recv() {
            dirty |= app.handle(more);
        }
        if app.quit {
            break;
        }
        if dirty {
            terminal.draw(|f| ui::draw(f, app))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("250ms"), Some(Duration::from_millis(250)));
        assert_eq!(parse_duration("1s"), Some(Duration::from_secs(1)));
        assert_eq!(parse_duration("0.5s"), Some(Duration::from_millis(500)));
        assert_eq!(parse_duration("2"), Some(Duration::from_secs(2)));
        assert_eq!(parse_duration("500"), Some(Duration::from_millis(500)));
        assert_eq!(parse_duration("abc"), None);
    }
}
