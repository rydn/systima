//! Screen layout and chrome (header, footer, overlays). Panels live in
//! their own modules and draw straight into the frame buffer.

mod cpu;
pub mod graph;
mod mem;
mod procs;

use std::fmt::Write as _;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

use crate::app::{App, Focus};
use crate::format;
use crate::input::{HELP_LINES, Mode};
use crate::theme::Theme;
use graph::{Series, Unit, render_dual};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let theme = app.theme;
    frame.render_widget(Block::new().style(theme.base()), area);
    if area.height < 3 || area.width < 20 {
        return;
    }

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame.buffer_mut(), app, header);

    if body.height >= 28 {
        let gh = (body.height * 3 / 10).clamp(8, 14);
        let [top, mid, rest] = Layout::vertical([
            Constraint::Length(gh),
            Constraint::Length(gh),
            Constraint::Min(5),
        ])
        .areas(body);
        let [cpu_a, mem_a] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(top);
        let [net_a, disk_a] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(mid);
        cpu::draw(frame.buffer_mut(), app, cpu_a);
        mem::draw(frame.buffer_mut(), app, mem_a);
        draw_net(frame.buffer_mut(), app, net_a);
        draw_disk(frame.buffer_mut(), app, disk_a);
        procs::draw(frame.buffer_mut(), app, rest);
    } else if body.height >= 16 {
        let gh = (body.height * 2 / 5).clamp(7, 10);
        let [top, rest] =
            Layout::vertical([Constraint::Length(gh), Constraint::Min(5)]).areas(body);
        let [cpu_a, mem_a, net_a] = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .areas(top);
        cpu::draw(frame.buffer_mut(), app, cpu_a);
        mem::draw(frame.buffer_mut(), app, mem_a);
        draw_net(frame.buffer_mut(), app, net_a);
        procs::draw(frame.buffer_mut(), app, rest);
    } else {
        procs::draw(frame.buffer_mut(), app, body);
    }

    draw_footer(frame.buffer_mut(), app, footer);

    match app.mode {
        Mode::Help => draw_help(frame.buffer_mut(), app, area),
        Mode::ConfirmKill => draw_confirm_kill(frame.buffer_mut(), app, area),
        _ => {}
    }
    app.frames += 1;
}

/// A bordered panel in the theme's style with a `╢ title ╟` cap.
pub fn panel<'a>(theme: &Theme, title: Vec<Span<'a>>, focused: bool) -> Block<'a> {
    let mut spans = Vec::with_capacity(title.len() + 2);
    spans.push(Span::styled("╢ ", theme.border_style(focused)));
    spans.extend(title);
    spans.push(Span::styled(" ╟", theme.border_style(focused)));
    Block::bordered()
        .border_type(theme.border_type)
        .border_style(theme.border_style(focused))
        .style(theme.panel_style())
        .title(Line::from(spans))
}

fn draw_header(buf: &mut Buffer, app: &App, area: Rect) {
    let theme = &app.theme;
    let bar = theme.header_bar();
    let sep = Span::styled(" ▸ ", theme.muted_style());
    let mut spans: Vec<Span> = Vec::with_capacity(16);
    spans.push(Span::styled(" ▓▒░ SYSTIMA ░▒▓ ", bar));
    spans.push(Span::styled(" ", theme.base()));
    spans.push(Span::styled(
        app.host.hostname.as_str(),
        theme.accent_style().add_modifier(Modifier::BOLD),
    ));
    spans.push(sep.clone());
    spans.push(Span::styled(app.host.os.as_str(), theme.muted_style()));
    spans.push(Span::styled(" ", theme.base()));
    spans.push(Span::styled(app.host.arch.as_str(), theme.muted_style()));

    let mut s = String::with_capacity(32);
    if let Some(snap) = app.snap.as_deref() {
        format::duration(&mut s, snap.uptime);
        spans.push(sep.clone());
        spans.push(Span::styled("up ", theme.muted_style()));
        spans.push(Span::styled(s.clone(), theme.text_style()));

        spans.push(sep.clone());
        spans.push(Span::styled("load ", theme.muted_style()));
        let n = app.ncpu() as f64;
        for (i, l) in snap.load.iter().enumerate() {
            let t = (l / n) as f32;
            let st = Style::new().fg(theme.color(theme.by_load(t)));
            spans.push(Span::styled(format!("{l:.2}"), st));
            if i < 2 {
                spans.push(Span::styled(" ", theme.base()));
            }
        }

        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("{} cpu", snap.cores.len()),
            theme.text_style(),
        ));

        if let Some(t) = snap
            .temps
            .iter()
            .map(|t| t.celsius)
            .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))
        {
            spans.push(sep.clone());
            let st =
                Style::new().fg(theme.color(theme.by_load(((t - 40.0) / 60.0).clamp(0.0, 1.0))));
            spans.push(Span::styled(format!("{t:.0}°C"), st));
        }
    }
    if app.paused {
        spans.push(Span::styled(
            "  ▌▌ PAUSED ",
            theme.warn_style().add_modifier(Modifier::REVERSED),
        ));
    }
    buf.set_line(area.x, area.y, &Line::from(spans), area.width);

    // Right side: sampling interval and the theme name.
    s.clear();
    let _ = write!(
        s,
        " {} ▸ {:.2}s ",
        theme.palette.name(),
        app.interval.as_secs_f32()
    );
    let w = s.chars().count() as u16;
    if area.width > w + 40 {
        buf.set_string(area.right() - w, area.y, &s, theme.muted_style());
    }
}

fn draw_footer(buf: &mut Buffer, app: &App, area: Rect) {
    let theme = &app.theme;
    let key = theme.accent_style().add_modifier(Modifier::BOLD);
    let txt = theme.muted_style();
    let left: Line = match app.mode {
        Mode::Filter => Line::from(vec![
            Span::styled(" / ", key),
            Span::styled(
                app.filter.as_str(),
                theme.text_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled("▏", theme.accent2_style()),
            Span::styled("  Enter apply · Esc clear", txt),
        ]),
        _ => {
            if let Some(status) = app.status_text() {
                Line::from(vec![
                    Span::styled(" ▸ ", theme.accent2_style()),
                    Span::styled(status, theme.accent2_style()),
                ])
            } else {
                let mut v = Vec::with_capacity(20);
                for (k, d) in [
                    ("?", "help"),
                    ("c m p n", "sort"),
                    ("/", "filter"),
                    ("x", "kill"),
                    ("[ ]", "iface"),
                    ("␣", "pause"),
                    ("+ -", "rate"),
                    ("t", "theme"),
                    ("q", "quit"),
                ] {
                    v.push(Span::styled(" ", txt));
                    v.push(Span::styled(k, key));
                    v.push(Span::styled(" ", txt));
                    v.push(Span::styled(d, txt));
                    v.push(Span::styled(" ", txt));
                }
                Line::from(v)
            }
        }
    };
    buf.set_line(area.x, area.y, &left, area.width);

    // Right: our own footprint — the monitor should not be the load.
    if let Some(snap) = app.snap.as_deref() {
        let mut s = String::with_capacity(48);
        let mut rss = String::with_capacity(8);
        format::bytes(&mut rss, snap.self_rss);
        let _ = write!(
            s,
            " systima {:.1}% {} ▸ sample {:.1}ms ▸ {} tasks ",
            snap.self_cpu,
            rss,
            snap.sample_cost.as_secs_f32() * 1000.0,
            snap.procs.len()
        );
        let w = s.chars().count() as u16;
        if area.width > w + 30 {
            buf.set_string(area.right() - w, area.y, &s, txt);
        }
    }
}

fn draw_net(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme;
    let iface = app.iface.as_deref().unwrap_or("all");
    let n = app.snap.as_ref().map(|s| s.net.len()).unwrap_or(0);
    let title = vec![
        Span::styled("NET", theme.title_style()),
        Span::styled(" ▸ ", theme.muted_style()),
        Span::styled(iface, theme.accent_style()),
        Span::styled(format!(" ({n} ifaces)"), theme.muted_style()),
    ];
    let block = panel(&theme, title, app.focus == Focus::Net);
    let inner = block.inner(area);
    block.render(area, buf);
    let a = Series {
        glyph: "▼",
        label: "RX",
        data: &app.hist.rx,
        scale: &app.scale_rx,
        color: theme.accent,
    };
    let b = Series {
        glyph: "▲",
        label: "TX",
        data: &app.hist.tx,
        scale: &app.scale_tx,
        color: theme.accent2,
    };
    let cols = render_dual(buf, inner, &theme, Unit::Rate, a, b);
    if cols > 0 {
        app.net_width = cols;
    }
}

fn draw_disk(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme;
    let mut title = vec![Span::styled("DISK", theme.title_style())];
    if let Some(d) = app.snap.as_ref().and_then(|s| s.disks.first()) {
        let used = d.total.saturating_sub(d.available);
        let pct = if d.total > 0 {
            (used as f64 / d.total as f64 * 100.0) as u16
        } else {
            0
        };
        let mut t = String::with_capacity(8);
        format::bytes(&mut t, d.total);
        let mut u = String::with_capacity(8);
        format::bytes(&mut u, used);
        title.push(Span::styled(" ▸ ", theme.muted_style()));
        title.push(Span::styled(d.mount.clone(), theme.accent_style()));
        title.push(Span::styled(format!(" {u}/{t} "), theme.muted_style()));
        let st = Style::new().fg(theme.color(theme.by_load(pct as f32 / 100.0)));
        title.push(Span::styled(format!("{pct}%"), st));
    }
    let block = panel(&theme, title, app.focus == Focus::Disk);
    let inner = block.inner(area);
    block.render(area, buf);
    let a = Series {
        glyph: "▼",
        label: "RD",
        data: &app.hist.disk_r,
        scale: &app.scale_dr,
        color: theme.accent,
    };
    let b = Series {
        glyph: "▲",
        label: "WR",
        data: &app.hist.disk_w,
        scale: &app.scale_dw,
        color: theme.accent2,
    };
    let cols = render_dual(buf, inner, &theme, Unit::Rate, a, b);
    if cols > 0 {
        app.disk_width = cols;
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_help(buf: &mut Buffer, app: &App, area: Rect) {
    let theme = &app.theme;
    let h = HELP_LINES.len() as u16 + 4;
    let rect = centered(area, 64, h);
    Clear.render(rect, buf);
    let block = panel(theme, vec![Span::styled("HELP", theme.title_style())], true);
    let inner = block.inner(rect);
    block.render(rect, buf);
    let mut lines: Vec<Line> = Vec::with_capacity(HELP_LINES.len() + 2);
    lines.push(Line::from(Span::styled(
        "  systima ▸ cyberpunk system monitor",
        theme.accent_style().add_modifier(Modifier::BOLD),
    )));
    for (k, d) in HELP_LINES {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {k:<14}"),
                theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(*d, theme.text_style()),
        ]));
    }
    lines.push(Line::from(Span::styled(
        "  any key to close",
        theme.muted_style(),
    )));
    Paragraph::new(lines)
        .style(theme.panel_style())
        .render(inner, buf);
}

fn draw_confirm_kill(buf: &mut Buffer, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some((pid, name)) = &app.kill_target else {
        return;
    };
    let rect = centered(area, 56, 5);
    Clear.render(rect, buf);
    let block = panel(
        theme,
        vec![Span::styled("KILL", theme.danger_style())],
        true,
    );
    let inner = block.inner(rect);
    block.render(rect, buf);
    let lines = vec![
        Line::from(vec![
            Span::styled("  terminate ", theme.text_style()),
            Span::styled(
                format!("{pid}"),
                theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ", theme.text_style()),
            Span::styled(
                name.as_str(),
                theme.accent2_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ?", theme.text_style()),
        ]),
        Line::from(vec![
            Span::styled("  y", theme.warn_style()),
            Span::styled(" SIGTERM   ", theme.muted_style()),
            Span::styled("K", theme.danger_style()),
            Span::styled(" SIGKILL   ", theme.muted_style()),
            Span::styled("other", theme.accent_style()),
            Span::styled(" cancel", theme.muted_style()),
        ]),
    ];
    Paragraph::new(lines)
        .style(theme.panel_style())
        .render(inner, buf);
}
