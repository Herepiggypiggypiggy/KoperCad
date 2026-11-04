# PR 1927 evidence

source at git 3bd472f0df28676b256b2018405866fc2f83e4b1

Built from that exact head (HEAD.txt equals the sha). Pictures and results.json under /mnt/project-files/qa/pr1927-3bd472f0/ (one folder per run). Sizes 1366x768 and 1920x1080 for every scenario; check_layout ran in each.

| Scenario set | Runs | Result |
| --- | --- | --- |
| 9600-crane-parts-shapes (new: Simulate menu, Hoist in 3D, iso, front, side, hook) | 2 | 2 pass, 0 findings |
| all hoist-3d scenarios (1730, 1730b, 1730c, 1740, 1740b, 1750, 1760, 1760b ...) | 9 | 9 pass, 0 findings |
| winch-in-3d (1767, 1767b) | 2 | 2 pass, 0 findings |
| travel-sheet-and-3d (1791, 1791b) | 2 | 2 pass, 0 findings |
| crane-sim-example (1809, 1809b, the #1809 project) | 2 | 2 pass, 1 finding S4 (non-blocking) |

Baseline on clean main (origin/main, same scenario 1809b): the same S4 finding F-001, "2 pair(s) of controls may overlap" at 1920x1080. Not caused by this PR. Pictures in baseline-main-crane-sim/.

Simulate > Hoist in 3D entry: picture 9600-crane-parts-shapes-*-01-simulate-menu.png in the crane-parts folder.

Viewport: fills the window (about 330 px high at 1366 against 170 before) and frames the crane without floor and columns when it opens.
