# Learning and example coverage: roadmap

Scope: make sure every screen a user meets has a lesson that ticks, keep the lessons true to the program, and keep `docs/lessons/COVERAGE.md` current. Not the lesson framework (that is `handoff/learning`).

## Done
- Eleven coverage lessons, the Machines track and the coverage table (#1350).
- Dialogs stay left of the parked lesson card; standards-check closes Electrical inputs before Calculations (#1383, #1390).
- Learning critic fixes: Drawing standard anchors and real clicks, row drag quarter-pitch rule, wheel dwell, Page Up and Down with the pointer on the card (#1397).
- Lesson walk of all 16 new lessons on main (2026-10-22): 32 runs, 0 blocking (before #1370: 14).
- 2026-10-25 backlog: lessons for EMC net classes, EMC on the plate and Build order, the Live view current paths step, QA walks 200 to 203, anchors, `pick` solve, panel scroll-into-view.

## Next, in order
1. Recheck the other lessons against current main (full walk with the 200s style scenarios; the replay gate already passes).
2. Coverage batches still open in `docs/lessons/COVERAGE.md` section 3: `wire-numbering-in-depth`, `plc-lists`, `why-and-faults`, `find-your-way`, `bring-a-project-in`, `variants-and-options`, `ask-claude`.
3. Advanced lessons: multi-cabinet, PLC program.
4. Manual links per lesson (manual #1360 is merged).
5. EMC check window and the 3D EMC view: a lesson each once the visualisation lane settles.

## Open questions
- LC-35 (a lesson card cannot be clicked through an open modal), LC-36, suite notes S-2 to S-5 are not fixed.
- Held pushbutton in the Live view cannot be scripted, so no step ticks on it.
