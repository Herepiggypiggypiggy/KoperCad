# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793243931 (unix time), source at git dd60a6c29a.

App: 1920.0x1080.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Drive run at 1920: the radio steps drive the frequency drive and the motor, the drawn end limit stops the axis (611-drive-run-1920) | Simulate | 1920x1080 | pass | - | 6 |
| Drive run: the radio steps drive the frequency drive and the motor, the drawn end limit stops the axis (611-drive-run) | Simulate | 1366x768 | pass | - | 6 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
