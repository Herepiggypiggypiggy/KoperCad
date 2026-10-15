# Workspaces thread: handoff

Design: `docs/design/workspaces.md` (PR #438, draft). Plan is §10, decisions for other threads §11.
Repo herepiggypiggypiggy/kopercad. Coordinator told us Tadjiro chose "Menu and status bar" at 18:44Z: go with the design.

## PRs (stacked in this order; each branch contains the ones above it)
| Step | PR | Branch |
| --- | --- | --- |
| 1 design doc | #438 | claude/project-thread-xni9es (design READY from 0114 with notes, folded in) |
| 2 workspace enum, switcher, layouts | #453 | ...-workspace (main A READY, needs design 0114) |
| 3 menu placements in novo-workflow | #459 | ...-registry (main A READY) |
| 4 menu bar | #463 | ...-menubar (design B changes made: F10, widths, bar width; main B) |
| 5 status bar | #467 | ...-statusbar (design C, main C) |
| 8 Reports workspace | #475 | ...-reports (design B changes made; main B) |
Reviewers: routing in coordination/threads.md (design reviewers by PR parity, main reviewers by mod 3). Message them after each push.

## Next
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
