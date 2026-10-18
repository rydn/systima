---
name: ui/rendering-conventions
kind: decision
tags: [ui, ratatui, theme]
related: [file:src/ui/graph.rs, file:src/ui/mod.rs, file:src/theme.rs, file:src/app.rs, file:src/format.rs]
created: 2026-10-18T15:12:09.761Z
updated: 2026-10-18T15:12:09.761Z
---

UI conventions in systima: every panel is drawn by a function taking `&mut Buffer` (not ratatui's high-level Table/Sparkline/Chart) and writing cells directly, because the graphs scroll every tick and the high-level widgets allocate Rows/Spans per frame. `ui::panel(theme, title_spans, focused)` is the one Block factory (`╢ title ╟` caps, theme border type). `ui/graph.rs::HistoryGraph` is the bmon-style widget: left gutter of GUTTER=8 columns with ceiling / half / zero labels and `┤` ticks, block columns `█` plus a partial `▁▂▃▄▅▆▇` top cell, colour from `Theme::grad_at(row/height)` computed per row (outer loop rows, inner loop columns, so no per-frame Vec). Axis ceilings come from `app::Scale` (1-2-5-1024 ladder via `format::nice_ceiling`, hysteresis: grow immediately, shrink only after 5 frames below 40%), with the window width fed back from the UI (`app.net_width` / `app.disk_width`) after each draw. CPU history is stored in tenths of a percent (u64) with a fixed ceiling of 1000; memory uses `mem_total` as its ceiling. Colours are stored as `theme::Rgb` and converted through `Theme::color()`, which falls back to xterm-256 when `truecolor` is false (detected from COLORTERM / TERM_PROGRAM; Terminal.app has no truecolor). Themes: `cyberpunk` (default) and `matrix`; `t` cycles.
