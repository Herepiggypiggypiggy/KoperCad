# Workspaces thread: handoff

Branch `claude/project-thread-xni9es` (repo herepiggypiggypiggy/kopercad). Step 1 (design doc) is the open draft PR.
Read `docs/design/workspaces.md` first; its §10 is the plan and §11 the decisions needing other threads.

Coordinate with: App shell WS-01 (cse_01AYuFPxz6eQReQdtzYKLMmW, owns novo-app), Layout/branding
(cse_01UXbCeQW22y6tv74F2Xvbvm), Workflow (cse_01JHYmmTDsVHDL7pEab8MGwy, owns DESIGN.md and the keymap), the 3D threads.
Full roster: `coordination/threads.md`.

Traps: `app.rs` is 2400+ lines and busy, so new code goes in new files under `crates/novo-app/src/app/`.
`cabinet_open: bool` is the current workspace state. `Action` (17) and `COMMANDS` (~75, novo-workflow) are two
lists today. `RESERVED_SHORTCUTS` holds Ctrl+3/4/5 for drawer tabs. No build until the owner asks.
