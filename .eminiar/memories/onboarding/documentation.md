---
name: onboarding/documentation
kind: onboarding
tags: []
related: [file:.eminiar/docs.json, file:README.md]
created: 2026-10-18T15:14:19.598Z
updated: 2026-10-18T15:14:19.598Z
---

Registered documentation sources (manifest `.eminiar/docs.json`, committed):

- `ratatui` — ratatui 0.30 API: crate root, widgets, `Buffer`, `Block`, `Layout`, `style`, and the BREAKING-CHANGES.md from the ratatui repo (crate was split into ratatui-core / ratatui-widgets in 0.30; `ratatui::run`, `Frame::area`, `Buffer` indexing by `(x, y)`).
- `sysinfo` — sysinfo 0.39 API: `System`, `Process`, `ProcessRefreshKind`, `Networks`, `Disks`, `DiskRefreshKind`, `Components`. Key facts: `refresh_processes_specifics(ProcessesToUpdate, remove_dead, ProcessRefreshKind)`, `with_user(UpdateKind)`, `Component::temperature() -> Option<f32>`, `Disk::usage()` is a delta since the last refresh, `MINIMUM_CPU_UPDATE_INTERVAL` = 200 ms.
- `crossterm` — crossterm 0.29 root and `event` module (`event::read`, `KeyEventKind`).
- `readme` — the project README (usage, keys, measured performance table, architecture, macOS caveats).

Third-party services/APIs: none at runtime. GitHub Actions is used for CI only (`.github/workflows/ci.yml`); no GitHub API calls.

Local specs: none (no OpenAPI, no ADR folder). Design decisions are eminiar memories: `architecture/threading-and-data-flow`, `ui/rendering-conventions`, `performance/sampler-costs-macos`, `process/commit-policy`. The implementation plan that produced the codebase lives outside the repo (`~/.claude/plans/lovely-meandering-whisper.md`).

Rule: `em_docs {action:"lookup", query}` before any WebFetch/WebSearch for ratatui, sysinfo or crossterm questions; refresh with `em_docs {action:"sync"}` after bumping a crate version.
