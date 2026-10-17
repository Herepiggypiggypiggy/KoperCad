# 3D navigation and selection: handoff

State 2026-10-17: all queued PRs merged except #880 (READY at 101a6308, CI green, waiting for Lead A). Do not push to it.

Resume: `git fetch origin main && git checkout -B <new-branch> FETCH_HEAD`, then take the next item in ROADMAP.md. One PR per feature, no stacked PRs.

Who: Lead A (even PRs) session_01FiLvRBKEcNNHbQEGhYyL1S; Lead B (odd PRs) session_011NKUp2tgBn8uVWK6WHsc7k (check coordination/threads.md); reviewers R5 session_01DHLANQPyc634wMAeUYHDHz, R7 session_01N8N2TSt2bzQ9xY7X5AvpRU.

Traps:
- Register a command in four places: workbench.rs APP_COMMANDS, novo-workflow commands.rs, menus.rs PLACEMENTS, language.rs Dutch aliases (plus keymap if bound).
- `Globals` field order must match in render/mod.rs and scene.wgsl.
- Toolbar `BELOW = 140.0` clears panels; `view3d_move_tests` checks 1366x768 and 900x600.
- Merge conflicts are nearly always "keep both"; commands.rs and language.rs auto-merges can unbalance blocks.
- Build: `CARGO_INCREMENTAL=0`; disk is tight; run `cargo test -p novo-app -p novo-view3d -p novo-workflow --lib`.
- UI colours via `crate::theme::ui_palette()`.
