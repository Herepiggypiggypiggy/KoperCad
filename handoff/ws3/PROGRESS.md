# WS-03 Canvas: progress log

2026-10-14 13:00 UTC Built novo-canvas engine, renderer, widget, PageScene; opened PR #4.
2026-10-14 14:50 UTC PR #4 green on Linux and Windows (as_chunks fix for clippy 1.99).
2026-10-14 15:08 UTC Fixed review: potential rename and strip retag relabel; demo --stress mode; merged #11.
2026-10-14 15:17 UTC Added right-drag threshold, Escape ladder (workflow requests).
2026-10-14 15:21 UTC Re-review READY; fixed off-canvas right click nit; themable sheet/frame colours.
2026-10-14 15:28 UTC PR #4 merged as d91e961.
2026-10-14 16:31 UTC Opened PR #28: label picking, related highlight, pixel snap, shared symbol art.
2026-10-14 16:40 UTC #28: junction glyphs derived from topology (integration lead arbitration).
2026-10-14 16:45 UTC #28: removed Tab/orientation binding (interaction-model.md).
2026-10-14 16:58 UTC #28: SymbolArt::labels and tag_anchor for WS-06; head e0dc3e7; reviewer notified.
2026-10-14 17:00 UTC Wrote handoff files. Queued: page frame columns + title block (WS-06), xref labels (after #31).
2026-10-14 17:10 UTC Page frame columns + title block done locally (d9f1e9c, frame.patch); waits for #28 to merge.
2026-10-14 17:20 UTC #28: fixed tester F2 (terminal name only, ink beats text); head 924be9b; reviewer + tester notified. frame.patch rebased (4faa2b8).
2026-10-14 17:35 UTC Marks (DESIGN §5) and DESIGN §3 title block done locally on canvas-marks; patches in next-pr/. Waiting for #28 review and #35.
2026-10-14 17:16 UTC Gate 0 passed on Quadro P1000 (10k: 1135 fps avg / 222 1% low). 100k: 113 avg / 38.5 1% low, stutter queued.
2026-10-14 21:30 UTC Resumed: #28 and #47 merged. New queue from integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S (xref in scene, wgpu optional, sheets, Xvfb click).
2026-10-14 21:45 UTC #93 opened (head afacdc4): xref labels in the page scene (PageScene::set_notes), PDF prints them. Local CI green (627 tests).
2026-10-14 21:50 UTC Local branches ready, patches in next-pr/: canvas-wgpu-optional (gpu feature, novo-cli headless; click regression tests), canvas-sheets (set_sheet), canvas-perf (100k stutter: show() p99 20.9 -> 1.2 ms).
2026-10-14 21:50 UTC Xvfb click: does not reproduce in code (canvas and full-window tests select). This container's Xvfb ignores XTEST/XWarpPointer, pointer frozen at screen centre.
2026-10-14 21:59 UTC #93 merged. #103 opened (head c7f5b73): wgpu optional + click tests; reviewer B asked.
2026-10-14 22:15 UTC canvas-drawing ready locally (105d8ff): junction Plain for <3 wires (DESIGN §2), NoteAnchor, keep_color for warnings, strip names per run (F5). Outputs thread asked to drop its SheetLabel; mine (canvas-sheets) owns set_sheet.
2026-10-14 22:05 UTC #103 merged. drawing branch rebased; maps xref LabelAnchor (#102) to NoteAnchor.
2026-10-14 22:15 UTC #109 opened (head a567762): SheetContent/set_sheet; schematic n of m counts schematic pages only (design ruling). Reviewer B + design reviewer + outputs told. Next: perf, then drawing.
2026-10-14 22:15 UTC #109 head 3dfbc1f: sheets named 20a/20b (design reviewer CHANGES NEEDED item); both reviewers pinged.
2026-10-15 03:10 UTC Resumed after restart. #109 merged. #120 opened (head 95279c2 on main 89971f4): canvas perf. Reviewer B + lead told.
2026-10-15 03:20 UTC Drawing rebased on #120 (ffca612, local branch canvas-drawing-next, patch in next-pr/canvas-drawing/); un-ignores F5 gap_pdf_names_the_terminal_strip (passes). Local CI 683 passed. Opens as a PR once #120 merges.
2026-10-15 03:40 UTC canvas-reveal ready (451904b on drawing; patch next-pr/canvas-reveal/): CanvasView::reveal for jumps, moves only when the target is off screen; app jump + terminal proposals use it. 686 passed.
2026-10-15 03:10 UTC #120 READY (reviewer B) and merged as dca9a9a. Drawing PR rebased on main d7f70b8.
2026-10-15 03:20 UTC #126 READY from reviewer B (design review pending). Its 2 non-blocking notes fixed on canvas-reveal (plain junction pickable; tag label kept explicitly). Patches next-pr/canvas-reveal/.
2026-10-15 03:30 UTC #126 READY from design reviewer too. Its note 2 (two-row strips) fixed on canvas-reveal (3 commits). Plain junctions are pickable at their point only; a move tool that drags bends would need more.
2026-10-15 03:25 UTC Main reviewer CHANGES NEEDED on #126 (two-row strips). Fixed and pushed 1bf0920 (also pickable plain junctions). canvas-reveal now just the reveal commit on top.
2026-10-15 03:40 UTC #126 merged (READY from main reviewer, B, design). #144 opened (head c54805f on main 511cd18): jump reveal. Reviewer B, design, lead told.
2026-10-15 03:50 UTC #144 head 6eabbdf: added SheetText.bold/Label.bold (faux bold on canvas) for the outputs thread's report headers. novo-pdf maps TextBox::bold after merge.
2026-10-15 03:55 UTC #144 READY (B, design) on c54805f; pushed 6eabbdf (bold) + 87614b9 (reveal waits for target, reveal_area, reveal_margin). Delta re-checks requested.
2026-10-15 04:00 UTC #144 head 4f842a6 (bold 4%, fits cells). Delta re-checks requested. Outputs thread has the novo-pdf Helvetica-Bold side staged for after merge.
2026-10-15 04:10 UTC #144 merged. #154 opened (head bdc9043 on main 6fa1492): CanvasView::bold_family, app uses novo-theme strong family (Inter SemiBold).
2026-10-15 04:15 UTC #154 merged (bold face). Nothing open.
