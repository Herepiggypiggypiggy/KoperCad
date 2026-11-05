# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/target/debug/kopercad`, built 1793893241 (unix time), source at git e412f7cecb.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Crane model: pick a STEP crane of your own, see it in place of the generic crane, change a role, undo (9610-crane-model-picker) | Simulate | 1366x768 | pass | - | 9 |
| Crane model: pick a STEP crane of your own, see it in place of the generic crane, change a role, undo (9610-crane-model-picker) | Simulate | 1920x1080 | pass | - | 9 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
