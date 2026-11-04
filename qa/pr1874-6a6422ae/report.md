# QA PR 1874 (source at git 6a6422aecb)
Built from the code head in HEAD.txt (merge of main; the evidence commit after it only adds these files); screenshots in /mnt/project-files/qa/pr1874-6a6422ae/.
| Scenario | 1366x768 | 1920x1080 |
|---|---|---|
| 18740-calc-busbars (Busbar impedance tab: default run, 2 bars stacked, side by side with share and hottest bar, sweep, Copy as text, back to Circuits) | pass | pass |
| 186700-calc-mechanical (main's Mechanical tab, now fifth of six) | pass | pass |
| 7390 / 7391 / 7391b / 7392 help popups (open Calculations) | pass | pass |
Six-tab row (Circuits, Motor starters, Panel SCCR, Busbar impedance, Mechanical, Busbar heat) fits at 1366x768 without clipping: busbars/shots/18740-calc-busbars-1366x768-01-busbars-one-bar.png.
Findings: S4 only (top-bar "3D" tab under the docked window title at 1366).
