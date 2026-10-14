# WS-01 progress

2026-10-14 14:30 UTC Opened PR #3 (app shell, installer, CI).
2026-10-14 15:30 UTC Applied reviewer feedback on #3: static CRT, shortcut scoping, startup error dialog, build.rs target check.
2026-10-14 16:24 UTC Merged main into #3 to clear a Cargo.lock conflict (798193a). CI green on Linux, Windows and installer.
2026-10-14 16:36 UTC #3 merged as 11363c1.
2026-10-14 16:41 UTC Restarted branch claude/app-shell-x8aamm from main. Opened PR #34: storage, autosave, crash recovery, untitled projects on disk, canvas drawing.
2026-10-14 16:45 UTC Theme wiring for #20 committed locally (branch theme-wip, 00e9cf9), not pushed. Waiting for #20.
2026-10-14 16:46 UTC Pushed baf6e72 to #34: fixed two untitled-recovery edge cases WS-04 found, merged main (#25).
2026-10-14 16:57 UTC Asked the reviewer to review #34 at baf6e72. Asked WS-06 to have SchematicView return ChangeSets.
2026-10-14 16:58 UTC Gate 1 integration assigned to WS-01. Briefed the tester on the novo-app entry points.
2026-10-14 17:02 UTC Wrote the handoff files.
2026-10-14 17:01 UTC WS-06 changed SchematicView::show to return the frame's ChangeSet (#38 81c0caa). Adapter plan updated.
2026-10-14 17:11 UTC docs/DESIGN.md merged. Built the §3 Workbench locally (branch workbench-wip, 29ba4e2, on top of #34): top bar, outline pinned or flyout, HUD, toasts, focus mode, F11. No menu bar or status bar. Saved as workbench.patch. Opens as a new PR from main after #34 merges.
2026-10-14 17:20 UTC Workbench branch, second commit (8c69f1b, local only): start-up reopens the last project, start panel (New, Open, Recent) when nothing is open, NovoApp::execute/document/current_page for the tester, Ctrl+3/4/5 reserved. workbench.patch now holds both commits. Design criteria from the workflow thread (session_01JHYmmTDsVHDL7pEab8MGwy) still open: Open example, issue pill, new projects default to Documents\NovoElectric (needs #43), drawer keeps the canvas centre fixed.
2026-10-14 17:35 UTC Lead agreed: no save prompts. Workbench branch commit 118d44d: saved projects write to their own file 0.5 s after each change and on close; untitled closed with work is kept as kept-*.novo (start panel "UNSAVED"); start-up reopens settings.last_open (untitled included). workbench.patch now holds 3 commits.
2026-10-14 19:20 UTC Fixed reviewer's CHANGES NEEDED on #46 (73e8194): a failed write on New/Open/Close keeps the project open and bound to its file (BackendError::NotWritten); a failing continuous save still autosaves the recovery file; retry stays at 30 s while failing; message history capped. Merged main. Regression tests added.
2026-10-14 19:18 UTC Fixed design review CHANGES NEEDED on #46: the breadcrumb's page link now opens the outline flyout (it was closed again in the same frame by the clicked-elsewhere check); regression test added. Review follow-ups listed in ROADMAP.
