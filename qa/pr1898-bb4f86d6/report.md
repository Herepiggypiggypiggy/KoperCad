# QA evidence, source at git bb4f86d6fabd3d501e723fab857dab70a443948b

Scenarios 18970, 18975 and 18980 (copies of #1887's 16176, 230 and 380 with the waits this fix needs; #1887 owns the originals). Run: scripts/qa.sh run <the three files> --size 1366x768 --size 1920x1080. 18975 declares only 1366x768 in its own @sizes.

**QA gate: PASS** (5 run(s), 0 failed, 0 blocking finding(s))

| Scenario | 1366x768 | 1920x1080 |
|---|---|---|
| 18970-heat-refuses-20m-enclosure-pr1898 | pass | pass |
| 18975-workpack-out-of-date-after-cooler-layout-pr1898 | pass | - |
| 18980-3d-cooling-advice-2d-vs-3d-comparison-pr1898 | pass | pass |

Honest note: an earlier identical run of this head had 18980 fail once at 1366x768 on click_text Quick (a timing race with the automatic Heat run); this rerun passed. Pictures: /mnt/project-files/qa/pr1898-bb4f86d6/.
