# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/target/debug/kopercad`, built 1793795961 (unix time), source at git 5343f81f61.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| A job that did not work is a triangle on the status bar with Retry and Dismiss (1366x768) (9540-shell-job-flag-1366x768) | Shell | 1366x768 | pass | - | 3 |
| A job that did not work is a triangle on the status bar with Retry and Dismiss (1920x1080) (9541-shell-job-flag-1920x1080) | Shell | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
