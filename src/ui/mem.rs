//! Memory panel: RAM/swap gauges on the left, used-memory history on the right.

use std::fmt::Write as _;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use super::graph::{HistoryGraph, Unit};
use super::panel;
use crate::app::App;
use crate::format;
use crate::theme::Theme;

const BAR_W: u16 = 12;
const LEFT_W: u16 = 4 + BAR_W + 6; // "RAM " + bar + " 100%"

pub fn draw(buf: &mut Buffer, app: &App, area: Rect) {
    let theme = &app.theme;
    let snap = app.snap.as_deref();
    let (total, used, avail, swap_t, swap_u) = snap
        .map(|s| {
            (
                s.mem_total,
                s.mem_used,
                s.mem_available,
                s.swap_total,
                s.swap_used,
            )
        })
        .unwrap_or((0, 0, 0, 0, 0));

    let mut a = String::with_capacity(8);
    let mut b = String::with_capacity(8);
    format::bytes(&mut a, used);
    format::bytes(&mut b, total);
    let t = ratio(used, total);
    let title = vec![
        Span::styled("MEM", theme.title_style()),
        Span::styled(" ▸ ", theme.muted_style()),
        Span::styled(
            a.clone(),
            Style::new()
                .fg(theme.color(theme.by_load(t)))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" / {b}"), theme.muted_style()),
    ];
    let block = panel(theme, title, false);
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.height == 0 || inner.width < 12 {
        return;
    }

    let show_left = inner.width >= LEFT_W + 12;
    if show_left {
        let x = inner.x;
        let mut y = inner.y;
        let bottom = inner.bottom();
        gauge(buf, theme, x, y, "RAM", t, &mut a);
        y += 1;
        if y < bottom {
            format::bytes(&mut a, used);
            kv(buf, theme, x, y, "used ", &a);
            y += 1;
        }
        if y < bottom {
            format::bytes(&mut a, avail);
            kv(buf, theme, x, y, "avail", &a);
            y += 1;
        }
        if y < bottom {
            format::bytes(
                &mut a,
                total
                    .saturating_sub(used)
                    .saturating_sub(avail.min(total.saturating_sub(used))),
            );
            // On macOS "available" includes reclaimable cache; show the strictly free remainder too.
            let free = total.saturating_sub(used);
            format::bytes(&mut a, free);
            kv(buf, theme, x, y, "free ", &a);
            y += 1;
        }
        if y < bottom && swap_t > 0 {
            gauge(buf, theme, x, y, "SWP", ratio(swap_u, swap_t), &mut a);
            y += 1;
            if y < bottom {
                format::bytes(&mut a, swap_u);
                format::bytes(&mut b, swap_t);
                let mut s = String::with_capacity(16);
                let _ = write!(s, "{a}/{b}");
                kv(buf, theme, x, y, "swap ", &s);
            }
        } else if y < bottom {
            kv(buf, theme, x, y, "swap ", "none");
        }
    }

    let gx = if show_left {
        inner.x + LEFT_W + 1
    } else {
        inner.x
    };
    if gx < inner.right() {
        let g = Rect {
            x: gx,
            y: inner.y,
            width: inner.right() - gx,
            height: inner.height,
        };
        HistoryGraph::new(&app.hist.mem, total.max(1), Unit::Bytes, theme).render(g, buf);
    }
}

fn ratio(num: u64, den: u64) -> f32 {
    if den == 0 {
        0.0
    } else {
        (num as f64 / den as f64) as f32
    }
}

fn gauge(
    buf: &mut Buffer,
    theme: &Theme,
    x: u16,
    y: u16,
    label: &str,
    t: f32,
    scratch: &mut String,
) {
    let t = t.clamp(0.0, 1.0);
    let color = Style::new().fg(theme.color(theme.by_load(t)));
    buf.set_string(
        x,
        y,
        label,
        theme.accent_style().add_modifier(Modifier::BOLD),
    );
    let filled = (t * BAR_W as f32).round() as u16;
    for i in 0..BAR_W {
        let cell = &mut buf[(x + 4 + i, y)];
        if i < filled {
            cell.set_symbol("█").set_style(color);
        } else {
            cell.set_symbol("░").set_style(theme.border_style(false));
        }
    }
    scratch.clear();
    let _ = write!(scratch, " {:>3}%", (t * 100.0).round() as u32);
    buf.set_string(x + 4 + BAR_W, y, scratch, color);
}

fn kv(buf: &mut Buffer, theme: &Theme, x: u16, y: u16, key: &str, val: &str) {
    buf.set_string(x + 4, y, key, theme.muted_style());
    buf.set_string(x + 4 + key.len() as u16 + 1, y, val, theme.text_style());
}
