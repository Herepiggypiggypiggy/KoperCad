# QA evidence, source at git 34c2299dade7fe244ddf9fb1788414edfde1e0e2

Scenarios 18970, 18975, 18980 (copies of #1887's 16176, 230, 380 with the waits this fix needs; #1887 owns the originals) and 85-cooling-advice (its expectation of the removed on-screen sentence now reads "Best of"). Run: scripts/qa.sh run <the four files> --size 1366x768 --size 1920x1080 on this head. 18975 declares only 1366x768.

**QA gate: PASS** (7 run(s), 0 failed, 0 blocking finding(s))

| Scenario | 1366x768 | 1920x1080 |
|---|---|---|
| 18970-heat-refuses-20m-enclosure-pr1898 | pass | pass |
| 18975-workpack-out-of-date-after-cooler-layout-pr1898 | pass | - |
| 18980-3d-cooling-advice-2d-vs-3d-comparison-pr1898 | pass | pass |
| 85-cooling-advice | pass | pass |

R6 findings: (1) 1366: the Heat card now stays left of the docked Cooling advice list and the comparison is fully readable (18980 picture 02-comparison); (2) comparison and footer explanations are behind a ? (visible: header with "two models, not a measurement", the numbers, "Largest gap", "Model estimate, not validated"); (3) refusal notice is one line ("Cannot simulate: a side is over 20 m"). Pictures: /mnt/project-files/qa/pr1898-34c2299d/.
