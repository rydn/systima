//! CPU panel: per-core load bars on the left, total-CPU history on the right.

use std::fmt::Write as _;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use super::graph::{HistoryGraph, Unit};
use super::panel;
use crate::app::App;

pub fn draw(buf: &mut Buffer, app: &App, area: Rect) {
    let theme = &app.theme;
    let snap = app.snap.as_deref();
    let total = snap.map(|s| s.cpu_total).unwrap_or(0.0);
    let ncores = snap.map(|s| s.cores.len()).unwrap_or(app.host.ncpu);

    let mut title = vec![
        Span::styled("CPU", theme.title_style()),
        Span::styled(" ▸ ", theme.muted_style()),
        Span::styled(
            format!("{total:>5.1}%"),
            Style::new()
                .fg(theme.color(theme.by_load(total / 100.0)))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" ▸ {ncores} cores"), theme.muted_style()),
    ];
    if let Some(f) = snap.map(|s| s.freq_mhz).filter(|f| *f > 0) {
        title.push(Span::styled(
            format!(" ▸ {:.1} GHz", f as f64 / 1000.0),
            theme.muted_style(),
        ));
    }
    let block = panel(theme, title, false);
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.height == 0 || inner.width < 12 {
        return;
    }

    // --- per-core bars ----------------------------------------------------
    let cores: &[f32] = snap.map(|s| s.cores.as_slice()).unwrap_or(&[]);
    let h = inner.height as usize;
    let mut left_w: u16 = 0;
    if !cores.is_empty() {
        let cols = cores.len().div_ceil(h);
        // "12 ████████ 100%" → 2 + 1 + bar + 1 + 4
        let budget = (inner.width as usize * 55 / 100) / cols.max(1);
        let bar_w = if budget >= 16 {
            8
        } else if budget >= 12 {
            4
        } else {
            0
        };
        if bar_w > 0 {
            let cell_w = 2 + 1 + bar_w + 1 + 4 + 1;
            left_w = (cols * cell_w) as u16;
            let mut s = String::with_capacity(8);
            for (i, &usage) in cores.iter().enumerate() {
                let col = i / h;
                let row = i % h;
                let x = inner.x + (col * cell_w) as u16;
                let y = inner.y + row as u16;
                let t = (usage / 100.0).clamp(0.0, 1.0);
                let color = Style::new().fg(theme.color(theme.by_load(t)));
                s.clear();
                let _ = write!(s, "{i:>2} ");
                buf.set_string(x, y, &s, theme.muted_style());
                let filled = (t * bar_w as f32).round() as usize;
                for b in 0..bar_w {
                    let cell = &mut buf[(x + 3 + b as u16, y)];
                    if b < filled {
                        cell.set_symbol("█").set_style(color);
                    } else {
                        cell.set_symbol("░").set_style(theme.border_style(false));
                    }
                }
                s.clear();
                let _ = write!(s, " {:>3}%", usage.round() as u32);
                buf.set_string(x + 3 + bar_w as u16, y, &s, color);
            }
        }
    }

    // --- history ------------------------------------------------------------
    let gx = inner.x + left_w + u16::from(left_w > 0);
    if gx < inner.right() {
        let g = Rect {
            x: gx,
            y: inner.y,
            width: inner.right() - gx,
            height: inner.height,
        };
        HistoryGraph::new(&app.hist.cpu, 1000, Unit::Percent, theme).render(g, buf);
    }
}
