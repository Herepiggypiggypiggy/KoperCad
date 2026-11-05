# QA evidence, source at git b9e2a8509bf399578d9bb7e4d4026584e838252b

The PR's own scenarios on the exact code head b9e2a850 (after the merge of main and main's Dialog::tall): 18970, 18975, 18980 and 85-cooling-advice, both sizes (18975 declares 1366x768 only).

**QA gate: PASS** (7 run(s), 0 failed, 0 blocking finding(s))

| Scenario | 1366x768 | 1920x1080 |
|---|---|---|
| 18970-heat-refuses-20m-enclosure-pr1898 | pass | pass |
| 18975-workpack-out-of-date-after-cooler-layout-pr1898 | pass | - |
| 18980-3d-cooling-advice-2d-vs-3d-comparison-pr1898 | pass | pass |
| 85-cooling-advice | pass | pass |

Pictures: 18975 03-applied/04-stale (Cooler layout applied, stale work-card strip), 18980 02-comparison at 1366 (docked Cooling advice list readable, one ? in the title bar, comparison block and Model estimate line visible). Full set in /mnt/project-files/qa/pr1898-b9e2a850-scenarios/.
