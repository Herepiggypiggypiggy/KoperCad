# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793624097 (unix time), source at git 03dc31cd8f.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| 3D Heat: Heat flow modes, device paths and the cloud that moves with the air (18150-3d-heat-flow-modes-and-moving-cloud) | 3D | 1366x768 | pass | - | 3 |
| 3D Heat: Heat flow modes, device paths and the cloud that moves with the air (18150-3d-heat-flow-modes-and-moving-cloud) | 3D | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
