# Evidence: Calculations follow-up (source at git 8ed8992b75)

Head: see HEAD.txt (8ed8992b751253c4026753267afb148a23f8243a). Built from this head with the lesson embed touched.
Pictures (not in the repo): /mnt/project-files/qa/followup-busbar-8ed8992b/{busbars,mechanical}/shots/

| Scenario | 1366x768 | 1920x1080 |
|---|---|---|
| 18740-calc-busbars | pass | pass |
| 186700-calc-mechanical | pass | pass |
| 186701-calc-mechanical-no-supply | pass | pass |
| 186702/186703 bad input | pass | pass |

0 blocking findings. The only findings are S4 "controls may overlap" from the layout heuristic; the pictures show the "?" next to its field, no overlap.
Report for 18740 is report.md; the mechanical report is in the mount folder above.
