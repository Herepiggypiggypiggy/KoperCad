# PR 1813 evidence: Cabinet batch select, group drag, Align row

Code head `eb9330f0cbd92be727f1e679229b64e2a1801979` (HEAD.txt). Binary built from that source after the commit
(report.md: "source at git eb9330f0cb"), debug build, Linux, Xvfb. Scenarios `18132*` were added after it (scenario files
only, no code change) and run with the same code (report-18132.md). Pictures: `shots/`, `shots-18132/` in this folder.
Only HEAD.txt, report.md, report-18132.md and this file are committed in the repo.

Result: all 6 new scenario files pass at 1366x768 and 1920x1080, 0 failed. Existing Cabinet scenarios 770, 13-journey-cabinet,
1640, 1711, 15, 58 pass at both sizes; 1700 passes at 1920. `79-cabinet-rail-snap` and `1700-cabinet-sites` at 1366x768 fail
the same way on current main (baseline-main-79-1700-summary.md: same `55, 210 mm` / `Mounted` misses, they click a pixel that
now lands on a rail grip), so they are not caused by this PR. S4 at 1920 in report.md is the Inspector overlay over the
Unplaced list, present before.

| Check | Scenario | Pictures |
| --- | --- | --- |
| Box select, then group drag (before / after), undo and redo restore all three | 18130, 18130b | 01-before, 02-box-selected, 03-group-dragged, undone-whole-group, redone-whole-group |
| Shift-click, then drag: both move, one undo | 18132, 18132b | both-moved, undone-both |
| Align row with 2 selected (Spread greyed, hover says "Needs 3 selected devices") and with 3 | 18131, 18131b, 18132 | two-selected, three-selected, spread-disabled-reason |
| Align Bottom is one undo step | 18131, 18131b | aligned-bottom, align-undone |
| Ctrl+A | 18131, 18131b | select-all |
| Arrow nudge: Right moves one grid step, Down moves only devices not on a rail | 18131, 18131b | nudged-right, nudged-down |

Where and keys: Cabinet workspace. Drag on empty plate for a box; Shift-click adds; drag any selected device to move all;
Ctrl+A; arrows (Shift = 1 mm); the Align row docks under the plate while 2 or more are selected (it takes its own room, so it
does not float over the plate and sits above the status bar). Every control has a word on it.

Found by this QA: showing and hiding the Align row renumbered the canvas widget, so a drag that began while it came or went
was lost. The canvas now has a fixed id (`push_id`).
