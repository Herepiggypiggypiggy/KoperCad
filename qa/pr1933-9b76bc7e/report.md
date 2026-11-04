# PR 1933 evidence

source at git 9b76bc7e103576229cc7477e0a66370454ed0689

Built and run from that exact head (HEAD.txt equals the sha). Pictures and results.json: /mnt/project-files/qa/pr1933-9b76bc7e/ (one folder per run). Both sizes, 1366x768 and 1920x1080; check_layout ran in each.

| Scenario set | Runs | Result |
| --- | --- | --- |
| 9600-crane-parts-shapes (Simulate menu, Hoist in 3D, iso, front, side, hook) | 2 | 2 pass, 0 findings |
| all hoist-3d scenarios | 9 | 9 pass, 0 findings |
| winch-in-3d | 2 | 2 pass, 0 findings |
| travel-sheet-and-3d | 2 | 2 pass, 0 findings |
| crane-sim-example (the #1809 project) | 2 | 2 pass, 1 finding S4 (non-blocking) |

Baseline on clean main (same scenario 1809b): the identical S4 finding F-001 "2 pair(s) of controls may overlap" at 1920x1080 (baseline-main-crane-sim/). Not caused by this PR.

Simulate > Hoist in 3D entry: 9600-crane-parts-shapes-*-01-simulate-menu.png. The viewport fills the window and frames the crane on open.
