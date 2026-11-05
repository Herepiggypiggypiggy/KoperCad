# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 4 run(s): 4 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/tgt/debug/kopercad`, built 1793824146 (unix time), source at git 43ff1d9dae.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| PLC card dialog and overview: a bad layout is a triangle row with its reason in a popup (9480-plc-card-triangle-rows-1366) | Schematic | 1366x768 | pass | - | 2 |
| A refused terminal edit is a transient triangle in the canvas corner, not a toast (9481-refused-edit-triangle-1366) | Schematic | 1366x768 | pass | - | 3 |
| PLC card dialog and overview: a bad layout is a triangle row with its reason in a popup (9482-plc-card-triangle-rows-1920) | Schematic | 1920x1080 | pass | - | 2 |
| A refused terminal edit is a transient triangle in the canvas corner, not a toast (9483-refused-edit-triangle-1920) | Schematic | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
