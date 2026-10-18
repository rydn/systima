//! bmon-style history graph: block-character columns, auto-scaled axis
//! labels in a left gutter, and a vertical colour gradient so peaks glow.
//! Writes straight into the `Buffer`; no per-frame allocation beyond one
//! small label string.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::app::Scale;
use crate::format;
use crate::history::Ring;
use crate::theme::{Rgb, Theme};

const PARTIAL: [&str; 8] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇"];
/// Width of the axis-label gutter, including the tick column.
pub const GUTTER: u16 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Bytes per second.
    Rate,
    /// Plain bytes.
    Bytes,
    /// Tenths of a percent.
    Percent,
}

pub fn label(out: &mut String, unit: Unit, v: u64) {
    match unit {
        Unit::Rate => format::rate(out, v),
        Unit::Bytes => format::bytes(out, v),
        Unit::Percent => format::pct_whole(out, v),
    }
}

pub struct HistoryGraph<'a> {
    data: &'a Ring<u64>,
    ceiling: u64,
    unit: Unit,
    theme: &'a Theme,
    gutter: bool,
}

impl<'a> HistoryGraph<'a> {
    pub fn new(data: &'a Ring<u64>, ceiling: u64, unit: Unit, theme: &'a Theme) -> Self {
        Self {
            data,
            ceiling: ceiling.max(1),
            unit,
            theme,
            gutter: true,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn gutter(mut self, on: bool) -> Self {
        self.gutter = on;
        self
    }

    /// Number of sample columns this graph would show in `area`.
    pub fn columns(area: Rect, gutter: bool) -> usize {
        if gutter && area.width > GUTTER + 4 {
            (area.width - GUTTER) as usize
        } else {
            area.width as usize
        }
    }
}

impl Widget for HistoryGraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let theme = self.theme;
        let h = area.height as usize;
        let mut gx = area.x;
        let mut w = area.width;

        if self.gutter && area.width > GUTTER + 4 {
            let muted = theme.muted_style();
            let tick = theme.border_style(false);
            let tick_x = area.x + GUTTER - 1;
            for r in 0..area.height {
                buf[(tick_x, area.y + r)].set_symbol("│").set_style(tick);
            }
            let mut s = String::with_capacity(12);
            let mut put = |row: u16, v: u64, s: &mut String| {
                label(s, self.unit, v);
                let text_w = (GUTTER - 1) as usize;
                let len = s.chars().count().min(text_w);
                let x = area.x + (text_w - len) as u16;
                buf.set_stringn(x, area.y + row, s.as_str(), text_w, muted);
                buf[(tick_x, area.y + row)].set_symbol("┤").set_style(tick);
            };
            put(0, self.ceiling, &mut s);
            if h >= 3 {
                put((h / 2) as u16, self.ceiling / 2, &mut s);
            }
            if h >= 2 {
                put((h - 1) as u16, 0, &mut s);
            }
            gx = area.x + GUTTER;
            w = area.width - GUTTER;
        }

        let w = w as usize;
        let m = self.data.len().min(w);
        if m == 0 {
            return;
        }
        let x0 = gx as usize + (w - m);
        let total = (h * 8) as u128;
        let ceiling = self.ceiling as u128;

        for r in 0..h {
            let t = if h > 1 {
                r as f32 / (h - 1) as f32
            } else {
                1.0
            };
            let style = Style::new().fg(theme.color(theme.grad_at(t)));
            let y = area.y + (h - 1 - r) as u16;
            for (i, v) in self.data.iter_last(m).enumerate() {
                let e = ((v as u128 * total * 2 + ceiling) / (2 * ceiling)).min(total) as usize;
                let (full, part) = (e / 8, e % 8);
                let x = (x0 + i) as u16;
                if r < full {
                    buf[(x, y)].set_symbol("█").set_style(style);
                } else if r == full && part > 0 {
                    buf[(x, y)].set_symbol(PARTIAL[part]).set_style(style);
                }
            }
        }
    }
}

/// One series of a stacked RX/TX-style panel.
pub struct Series<'a> {
    pub glyph: &'a str,
    pub label: &'a str,
    pub data: &'a Ring<u64>,
    pub scale: &'a Scale,
    pub color: Rgb,
}

/// Two series stacked vertically, each with a one-line summary and its own
/// axis (bmon behaviour). Returns the number of sample columns drawn.
pub fn render_dual(
    buf: &mut Buffer,
    area: Rect,
    theme: &Theme,
    unit: Unit,
    a: Series,
    b: Series,
) -> usize {
    if area.height < 2 || area.width == 0 {
        return 0;
    }
    let top_h = area.height.div_ceil(2);
    let top = Rect {
        height: top_h,
        ..area
    };
    let bottom = Rect {
        y: area.y + top_h,
        height: area.height - top_h,
        ..area
    };
    let mut cols = 0;
    for (rect, s) in [(top, a), (bottom, b)] {
        if rect.height == 0 {
            continue;
        }
        render_series_title(buf, Rect { height: 1, ..rect }, theme, unit, &s);
        if rect.height > 1 {
            let g = Rect {
                y: rect.y + 1,
                height: rect.height - 1,
                ..rect
            };
            cols = HistoryGraph::columns(g, true);
            HistoryGraph::new(s.data, s.scale.ceiling, unit, theme).render(g, buf);
        }
    }
    cols
}

fn render_series_title(buf: &mut Buffer, line: Rect, theme: &Theme, unit: Unit, s: &Series) {
    let mut x = line.x;
    let right = line.right();
    let mut put = |text: &str, style: Style, x: &mut u16| {
        if *x >= right {
            return;
        }
        let w = (right - *x) as usize;
        buf.set_stringn(*x, line.y, text, w, style);
        *x += text.chars().count().min(w) as u16;
    };
    let head = Style::new()
        .fg(theme.color(s.color))
        .add_modifier(Modifier::BOLD);
    put(" ", head, &mut x);
    put(s.glyph, head, &mut x);
    put(" ", head, &mut x);
    put(s.label, head, &mut x);
    let mut v = String::with_capacity(12);
    let n = s.data.len().max(1);
    let cur = s.data.last().unwrap_or(0);
    let avg = s.data.avg_last(n).unwrap_or(0);
    let peak = s.data.max_last(n).unwrap_or(0);
    label(&mut v, unit, cur);
    put("  ", head, &mut x);
    put(
        &v,
        theme.accent_style().add_modifier(Modifier::BOLD),
        &mut x,
    );
    put("  avg ", theme.muted_style(), &mut x);
    label(&mut v, unit, avg);
    put(&v, theme.muted_style(), &mut x);
    put("  peak ", theme.muted_style(), &mut x);
    label(&mut v, unit, peak);
    put(&v, theme.accent2_style(), &mut x);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Palette;

    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn renders_columns_bottom_up_with_partials() {
        let theme = Theme::new(Palette::Cyberpunk, true);
        let mut ring = Ring::new(16);
        for v in [0u64, 4, 8, 12, 16, 20] {
            ring.push(v);
        }
        // Height 2 → 16 eighths. Values map to 0,4,8,12,16,16 eighths.
        let area = Rect::new(0, 0, 6, 2);
        let mut buf = Buffer::empty(area);
        HistoryGraph::new(&ring, 16, Unit::Bytes, &theme)
            .gutter(false)
            .render(area, &mut buf);
        let r = rows(&buf);
        assert_eq!(r[1], " ▄████");
        assert_eq!(r[0], "   ▄██");
    }

    #[test]
    fn right_aligns_sparse_data_and_draws_gutter() {
        let theme = Theme::new(Palette::Cyberpunk, true);
        let mut ring = Ring::new(16);
        ring.push(1024);
        let area = Rect::new(0, 0, GUTTER + 6, 3);
        let mut buf = Buffer::empty(area);
        HistoryGraph::new(&ring, 1024, Unit::Rate, &theme).render(area, &mut buf);
        let r = rows(&buf);
        // Labels: ceiling on top, half in the middle, zero at the bottom.
        assert!(r[0].starts_with(" 1.0K/s┤"), "{:?}", r[0]);
        assert!(r[1].starts_with(" 512B/s┤"), "{:?}", r[1]);
        assert!(r[2].starts_with("   0B/s┤"), "{:?}", r[2]);
        // Single sample sits in the right-most column, full height.
        for row in &r {
            assert_eq!(row.chars().last(), Some('█'));
            assert_eq!(row.chars().nth(GUTTER as usize), Some(' '));
        }
    }

    #[test]
    fn dual_panel_reports_columns() {
        let theme = Theme::new(Palette::Cyberpunk, true);
        let ring = Ring::new(8);
        let scale = Scale::new(1024);
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        let a = Series {
            glyph: "▼",
            label: "RX",
            data: &ring,
            scale: &scale,
            color: theme.accent,
        };
        let b = Series {
            glyph: "▲",
            label: "TX",
            data: &ring,
            scale: &scale,
            color: theme.accent2,
        };
        let cols = render_dual(&mut buf, area, &theme, Unit::Rate, a, b);
        assert_eq!(cols, 40 - GUTTER as usize);
        let r = rows(&buf);
        assert!(r[0].starts_with(" ▼ RX  0B/s"), "{:?}", r[0]);
        assert!(r[4].starts_with(" ▲ TX  0B/s"), "{:?}", r[4]);
    }
}
