# 3D navigation and selection: handoff

State 2026-10-17: everything is merged (#748 to #814, #880, #881, #882, #885, #895, #897, #898) except #886 (legend test, test-only; READY, waiting for R7's re-confirm on 7595c114 and Lead B). Do not push to it unless a lead asks for a merge of main.

Not verified: the Trackpad pans switch on real Windows hardware, and that the setting survives an app restart (code path read, not run). The 3D navigation list is empty: ask the coordinator what is next.

Resume: `git fetch origin main && git checkout -B <new-branch> FETCH_HEAD`, then take the next item in ROADMAP.md. One PR per feature, no stacked PRs.

Who: Lead A (even PRs) session_01FiLvRBKEcNNHbQEGhYyL1S; Lead B (odd PRs) session_011NKUp2tgBn8uVWK6WHsc7k (check coordination/threads.md); reviewers R5 session_01DHLANQPyc634wMAeUYHDHz, R7 session_01N8N2TSt2bzQ9xY7X5AvpRU.

Traps:
- Register a command in four places: workbench.rs APP_COMMANDS, novo-workflow commands.rs, menus.rs PLACEMENTS, language.rs Dutch aliases (plus keymap if bound).
- `Globals` field order must match in render/mod.rs and scene.wgsl.
- Toolbar `BELOW = 140.0` clears panels; `view3d_move_tests` checks 1366x768 and 900x600.
- Merge conflicts are nearly always "keep both"; commands.rs and language.rs auto-merges can unbalance blocks.
- Build: `CARGO_INCREMENTAL=0`; disk is tight; run `cargo test -p novo-app -p novo-view3d -p novo-workflow --lib`.
- UI colours via `crate::theme::ui_palette()`.
