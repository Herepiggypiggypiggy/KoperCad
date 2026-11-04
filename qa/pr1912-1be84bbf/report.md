# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 4 run(s): 4 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/tgt/debug/kopercad`, built 1793796304 (unix time), source at git 1be84bbfce.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| PLC overview and a bad card layout: triangles, not coloured sentences (9480-plc-overview-triangle-rows) | Schematic | 1366x768 | pass | - | 2 |
| PLC overview and a bad card layout: triangles, not coloured sentences (9480-plc-overview-triangle-rows) | Schematic | 1920x1080 | pass | - | 2 |
| A refused terminal edit is a triangle chip with the reason in it, not a toast (9481-refused-edit-chip) | Schematic | 1366x768 | pass | - | 3 |
| A refused terminal edit is a triangle chip with the reason in it, not a toast (9481-refused-edit-chip) | Schematic | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
