//! htop-style task table drawn directly into the buffer. Only the visible
//! window of the pre-sorted index list is touched each frame.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use super::panel;
use crate::app::{App, Focus};
use crate::input::SortKey;
use crate::snapshot::ProcState;

#[derive(Clone, Copy)]
struct Col {
    key: Option<SortKey>,
    name: &'static str,
    width: u16,
    right: bool,
}

const COLS: [Col; 8] = [
    Col {
        key: Some(SortKey::Pid),
        name: "PID",
        width: 7,
        right: true,
    },
    Col {
        key: Some(SortKey::User),
        name: "USER",
        width: 10,
        right: false,
    },
    Col {
        key: Some(SortKey::Cpu),
        name: "CPU%",
        width: 6,
        right: true,
    },
    Col {
        key: Some(SortKey::Mem),
        name: "MEM%",
        width: 6,
        right: true,
    },
    Col {
        key: None,
        name: "RSS",
        width: 6,
        right: true,
    },
    Col {
        key: None,
        name: "S",
        width: 1,
        right: false,
    },
    Col {
        key: Some(SortKey::Time),
        name: "TIME",
        width: 7,
        right: true,
    },
    Col {
        key: Some(SortKey::Name),
        name: "NAME",
        width: 0,
        right: false,
    },
];

pub fn draw(buf: &mut Buffer, app: &mut App, area: Rect) {
    let theme = app.theme;
    let n = app.procs.order.len();
    let total = app.procs.src.len();
    let running = app.snap.as_ref().map(|s| s.procs_running).unwrap_or(0);

    let mut title = vec![
        Span::styled("TASKS", theme.title_style()),
        Span::styled(" ▸ ", theme.muted_style()),
        Span::styled(
            format!("{total}"),
            theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" total ▸ {running} running ▸ sort "),
            theme.muted_style(),
        ),
        Span::styled(
            format!(
                "{}{}",
                app.sort.label(),
                if app.sort_desc { "▾" } else { "▴" }
            ),
            theme.accent2_style().add_modifier(Modifier::BOLD),
        ),
    ];
    if !app.filter.is_empty() {
        title.push(Span::styled(" ▸ filter ", theme.muted_style()));
        title.push(Span::styled(app.filter.clone(), theme.warn_style()));
        title.push(Span::styled(format!(" ({n})"), theme.muted_style()));
    }
    let block = panel(&theme, title, app.focus == Focus::Procs);
    let inner = block.inner(area);
    block.render(area, buf);
    if inner.height < 2 || inner.width < 20 {
        return;
    }

    // Visible window: header row + data rows.
    let rows_h = (inner.height - 1) as usize;
    app.table_height = rows_h;
    if app.selected < app.offset {
        app.offset = app.selected;
    } else if app.selected >= app.offset + rows_h {
        app.offset = app.selected + 1 - rows_h;
    }
    if n == 0 {
        app.offset = 0;
    } else {
        app.offset = app.offset.min(n - 1);
    }

    // Column layout: drop USER and TIME when narrow.
    let narrow = inner.width < 72;
    let mut xs: [(u16, u16); COLS.len()] = [(0, 0); COLS.len()];
    let mut x = inner.x;
    for (i, c) in COLS.iter().enumerate() {
        if narrow && matches!(c.name, "USER" | "TIME") {
            continue;
        }
        let w = if c.width == 0 {
            inner.right().saturating_sub(x)
        } else {
            c.width
        };
        xs[i] = (x, w);
        x += w + 1;
        if x >= inner.right() {
            break;
        }
    }

    // Header.
    let hdr = Style::new()
        .fg(theme.color(theme.bg))
        .bg(theme.color(theme.muted))
        .add_modifier(Modifier::BOLD);
    let hdr_active = Style::new()
        .fg(theme.color(theme.bg))
        .bg(theme.color(theme.accent2))
        .add_modifier(Modifier::BOLD);
    for cx in inner.x..inner.right() {
        buf[(cx, inner.y)].set_symbol(" ").set_style(hdr);
    }
    for (i, c) in COLS.iter().enumerate() {
        let (cx, w) = xs[i];
        if w == 0 {
            continue;
        }
        let active = c.key == Some(app.sort);
        put(
            buf,
            cx,
            inner.y,
            w,
            c.name,
            c.right,
            if active { hdr_active } else { hdr },
        );
    }

    // Rows.
    let base = theme.panel_style();
    let sel = theme.selection_style();
    let muted = theme.muted_style();
    let end = (app.offset + rows_h).min(n);
    for (r, oi) in (app.offset..end).enumerate() {
        let y = inner.y + 1 + r as u16;
        let Some((row, p)) = app.procs.proc_of(oi) else {
            break;
        };
        let selected = oi == app.selected;
        let row_style = if selected { sel } else { base };
        if selected {
            for cx in inner.x..inner.right() {
                buf[(cx, y)].set_symbol(" ").set_style(sel);
            }
        }
        let cpu_t = (row.cpu_val / 100.0).clamp(0.0, 1.0);
        let cpu_style = row_style.fg(theme.color(theme.by_load(cpu_t)));
        let mem_style = row_style
            .fg(theme.color(theme.by_load((row.mem_val as f64 / (8u64 << 30) as f64) as f32)));
        let state_style = match p.state {
            ProcState::Run => row_style
                .fg(theme.color(theme.good))
                .add_modifier(Modifier::BOLD),
            ProcState::Zombie | ProcState::Stop => row_style.fg(theme.color(theme.danger)),
            _ => {
                if selected {
                    sel
                } else {
                    muted
                }
            }
        };
        let state = [p.state.glyph() as u8];
        let state = std::str::from_utf8(&state).unwrap_or("?");
        let cells: [(&str, Style); 8] = [
            (&row.pid, if selected { sel } else { muted }),
            (&p.user, if selected { sel } else { muted }),
            (&row.cpu, cpu_style),
            (&row.mem_pct, mem_style),
            (&row.rss, row_style),
            (state, state_style),
            (&row.time, if selected { sel } else { muted }),
            (
                &p.name,
                if selected {
                    sel
                } else {
                    row_style.fg(theme.color(theme.text))
                },
            ),
        ];
        for (i, (text, style)) in cells.iter().enumerate() {
            let (cx, w) = xs[i];
            if w == 0 {
                continue;
            }
            put(buf, cx, y, w, text, COLS[i].right, *style);
        }
    }

    // Position indicator on the bottom border.
    if n > 0 && area.height >= 2 {
        let s = format!(" {}-{}/{} ", app.offset + 1, end, n);
        let w = s.len() as u16;
        if area.width > w + 4 {
            buf.set_string(
                area.right() - w - 2,
                area.bottom() - 1,
                &s,
                theme.muted_style(),
            );
        }
    }
}

/// Write `text` into a cell of width `w`, right- or left-aligned, clipped.
fn put(buf: &mut Buffer, x: u16, y: u16, w: u16, text: &str, right: bool, style: Style) {
    let len = text.chars().count();
    let w_us = w as usize;
    if right && len < w_us {
        buf.set_stringn(x + (w_us - len) as u16, y, text, len, style);
    } else {
        buf.set_stringn(x, y, text, w_us, style);
    }
}
