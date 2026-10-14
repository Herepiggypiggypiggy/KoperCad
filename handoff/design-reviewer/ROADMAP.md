# Design reviewer: roadmap

Scope: second reviewer for UI, visual and workflow PRs in NovoElectric.
- Review each against docs/DESIGN.md for coherence, elegance and flow, and flag anything copied from EPLAN or other CAD without a reason.
- Also check correctness, local CI (fmt, clippy -D warnings, tests, on a merge with main), the no-admin rule, and the real window under Xvfb.
- Verdicts go on the PR (COMMENT reviews with a verdict line) and to the coordinator.

## Done
- #20 theme: READY 2d60fdd (merged).
- #24 DESIGN.md: READY c7f78ba (merged). #43 DESIGN follow-ups: READY f63d0fa (merged).
- #38 schematic editor: reviewed through several heads; merged.
- #46 Workbench and #47 marks/page frame: reviewed; merged.
- #53 Gate 1 integration: four rounds (9956de4, 84eff50, 53f0e00, 67637b7). READY on 67637b7: proposals as dashed canvas marks, problem marks on the drawing, PDF next to the .novo with no silent overwrite, proposal hint shown.

## Next, in order
1. #53: READY posted on 67637b7. If a newer head appears before merge, re-review only the delta.
2. Review the UI follow-up PRs as they open (list in HANDOFF.md "Design notes still owed"). Order of value: problem/Issues outline mode, xref labels into the scene, drawer → drawing selection, Cables/Devices drawers, proposal dismiss, Space-held pan, inspector, tool dock.
3. Settle the hint-vs-Enter-target question (HANDOFF note 8) with the workflow thread and record it in DESIGN.md §5.
4. Windows check of single-click selection (HANDOFF note 7) once an on-device thread can run the app.

## Open questions
- Should Enter accept the proposal nearest the pointer (where the hint is) instead of a Tab-cycled index? DESIGN §5 says the hint sits on the nearest proposal, so the two should agree.
