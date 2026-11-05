# QA evidence, source at git 876a333e9706c607b7db2d631b2b34b043198288

Scenarios 18970, 18975, 18980 (copies of #1887's 16176, 230, 380 with the waits this fix needs; #1887 owns the originals) and 85-cooling-advice (expects "Best of" now). Run: scripts/qa.sh run <the four files> --size 1366x768 --size 1920x1080 on this head.
18975 has no 1920x1080 run because the scenario declares @sizes 1366x768 only (the work-card strip scenario was written for that size).

**QA gate: PASS** (7 run(s), 0 failed, 0 blocking finding(s))

| Scenario | 1366x768 | 1920x1080 |
|---|---|---|
| 18970-heat-refuses-20m-enclosure-pr1898 | pass | pass |
| 18975-workpack-out-of-date-after-cooler-layout-pr1898 | pass | - |
| 18980-3d-cooling-advice-2d-vs-3d-comparison-pr1898 | pass | pass |
| 85-cooling-advice | pass | pass |

Changes since 34c2299d (Main A/R3): the three Cooling advice explanations ("A try is one run...", the 2D-vs-3D depth sentence, the not-validated footer) go through window_help::add into the window's single "?" (picture 18980 02-comparison: one ? in the title bar, visible "two models, not a measurement" and "Model estimate, not validated"); the local why() helper is gone. The 20 m refusal notice has a ? that says where to fix it (Cabinet > Size enclosure). Pictures and full report.md of the run: /mnt/project-files/qa/pr1898-876a333e/.

Docked/floating window regression runs (panel.rs and panel_dock.rs changed): requested from the QA thread.
