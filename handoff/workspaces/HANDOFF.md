# Workspaces thread: handoff

Design: `docs/design/workspaces.md` (PR #438, draft). Plan is §10, decisions for other threads §11.
Repo herepiggypiggypiggy/kopercad. Coordinator told us Tadjiro chose "Menu and status bar" at 18:44Z: go with the design.

## PRs (2026-10-15 19:55 UTC; merge in this order, merge main into the next after each)
| Step | PR | Branch | State |
| --- | --- | --- | --- |
| 1 design doc | #438 | merged | |
| 2 workspace enum, switcher, layouts | #453 | merged | |
| 3 menu placements in novo-workflow | #459 | merged | |
| docs follow-up (widths, hub order, touch layout, anchors) | #515 | claude/project-thread-xni9es | design C READY with notes folded in |
| 4 menu bar | #463 | ...-menubar afb185d | main B + design B re-review after the main merge |
| 5 status bar | #467 | ...-statusbar 096198d | design C READY; stacked on #463 |
| 8 Reports workspace | #475 | ...-reports 33ff374 | design B items done; stacked on #467 |
Reviewers: routing in coordination/threads.md. Message them after each push.

## Next (not started; wind-down 19:45 UTC, usage at 80%)
still clippy: main's novo-interop `large_enum_variant` (sheets/fields.rs Parsed) fails `-D warnings`, not ours.
Workspace icons + segmented_icons from the design system (#501) replace the short chip names; Commissioning migrates onto Workspace::Commissioning (its thread); Alt+letter access not built.
Small registry PR after #459 merges: Revisions under File (PLACEMENTS in menus.rs). Merge up the stack as lower PRs squash-merge (rebase onto main and say so).
Trap found: egui MenuBar takes the whole row; menu_bar.rs gives it the width measured last frame.
Trap: COMMANDS rows must stay in GROUPS order (test); keys: Reports workspace uses Focus::Panel so the table gets arrows.
6 panel registry (drawer tabs per workspace, Alt+digit, F6, Reset layout); 7 palette menu-path column;
9 3D workspace (novo-view3d not on main yet), 10 Commissioning (novo-commissioning, design PR #399);
11 keys Ctrl+1..5 (Workflow must agree). UI integration thread (cse_01MQYH2ZYReC8dTZsMHYf4pc) has #440 filling a
project menu by id prefix: told them to register commands in COMMANDS and map into my menus.

## Traps
`app.rs` is huge and busy: new code in new files under crates/novo-app/src/app/. New COMMANDS rows need Dutch words
(language.rs) and a menu placement or PALETTE_ONLY reason (menus.rs), or tests fail. Cloud cannot render the app
(no libxkbcommon-x11 / GL), so UI needs a Windows hands-on look. No build until Tadjiro asks.
After a stacked PR below merges, rebase the next onto main and say so (threads.md rule).
