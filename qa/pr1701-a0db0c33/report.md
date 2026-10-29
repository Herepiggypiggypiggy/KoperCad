# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793274434 (unix time), source at git a0db0c3384.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Cabinet at 1366: More > Drill plan writes the hand drilling plan (16190-cabinet-drill-plan-export-1366) | Cabinet | 1366x768 | pass | - | 3 |
| Cabinet: Output > Export > Drill plan writes the hand drilling plan (16190-cabinet-drill-plan-export-1920) | Cabinet | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
