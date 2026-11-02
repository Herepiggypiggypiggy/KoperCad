# PR 1815 evidence

Head: 23a83ac682ab65e46a028f833e780e62feb3865f. The program was built at 25c0f403; the head adds one commit that only shifts the clicks of QA scenario 1602 (no app code), so the pictures are of this head's app.

Bar: 58 pt, tiles (22 pt icon, 11.5 pt caption). Every workspace tile, Undo and Redo carry a caption in words at 1366x768 and 1920x1080 (workspace names from 1300 pt wide; under that only the current name shows, under 760 pt icons only, name in the tooltip; the narrow rules are unchanged). The active workspace has an accent bar under the tile as well as the wash. Search at 1366 is the key name (Ctrl+K) in a taller button; Help is a tile only where the menu words are folded into the mark (under 1100 pt), at 1366/1920 the Help menu word is in the bar.

Runs (this folder), all 0 failed:
- 7390 (top bar tiles): shots bar-schematic, undo-tooltip-disabled ("Undo (Ctrl+Z): nothing to undo yet"), redo-tooltip-disabled, bar-cabinet; check_layout clean.
- 02-workspaces: every workspace tile.
- 18080 (3D layers rail), 1135 (Pages dock pane): taller bar leaves the 3D rail and the dock unclipped; layout checks clean.
- 1602 (outline multi-select): clicks shifted by 18 pt, passes.
- 602b: 7 scenarios matched, only 1602 failed (hard-coded click rows, fixed above).
Unit tests: top_bar, menu_bar width tests at 1100/1366/125-200% scaling (no overlap, breadcrumb clear of Search), rail fits at 1366x768, view3d cards. clippy -D warnings clean.
Not run: the full suite; other scenarios with hard-coded y coordinates below the bar may need the same 18 pt shift.
