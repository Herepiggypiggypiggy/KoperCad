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

## Coordinate sweep (R3 request)

31 scenarios using numeric click/drag/move coordinates, from schematic placing, wiring, Cabinet, 3D, Parts, Reports, docks and rails, run at both sizes (a scenario with a size in its name or @sizes runs at one). Folder sweep/. The 9 that were red were rerun on main (baseline-main/, main at the commit in its HEAD.txt) in the same way:

| Scenario | This PR | Main |
|---|---|---|
| 154-emc-net-classes-schematic | red | red (7 vs 2 findings, see note) |
| 210-learn-wire-test | red | red |
| 161-emc-bonding-3d-1366 | red | red |
| 621-part-picker-takes-typing | red | red |
| 1612-rails-list-refuses | red | red |
| 111-3d-heat-vents-hover | red | red |
| 100-cabinet-emc-left-out | red | pass: shift only |
| 1120-dock-inspector | red | pass: shift only |
| 1121-dock-by-dragging | red | pass: shift only |

The other 22 scenarios passed (07, 173, 331, 17103, 17104, 16172, 1700, 110, 1599, 701, 742, 1136, 7400, 1102, 18080, 730, 55, 59, 1135, 1101, 1105, 1602 earlier). The three shift-only ones are fixed in this PR (clicks moved +18, in 1120 the docked pane header +18 and the bottom-docked pane's anchor stays at the window foot) and pass at head 7a91cdd0e5d84f4bc72744e6c083bec341964bca (shifted/). Six were already red on main, so not caused by the bar. 154: the 2 S3 findings (`Set the class`, `Back to automatic`) are identical on main: its click at 800 310 no longer hits a wire there either, so that is old and not caused by the bar. The 5 extra findings were S4 layout-maybe, one per shot, all the same pair: on the page "3 Drive and signals" the breadcrumb text ended flush against the Search (Ctrl+K) button in the new bar (shot sweep/154-.../shots/...-02-finding-1.png). That was caused by this PR; fixed by leaving 10 pt more room before Search. After the fix 154 shows only the 2 findings main has (after-breadcrumb/). Content centred on the page moves 9 pt, not 18, and windows anchored at the foot do not move, so a mechanical +18 on every file would be wrong; I fixed by evidence instead.
Not run: the other ~235 coordinate scenarios; the lead runs the full suite from the merged result.
