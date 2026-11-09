# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/user/kopercad/target/debug/kopercad`, built 1794238790 (unix time), source at git 5ead1558bd.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| 3D heat: plate, wall and door maps colour the surfaces, and the opacity is lighter (the view from behind is covered by unit tests) (1991-3d-heat-surface-maps) | Heat | 1366x768 | pass | - | 5 |
| 3D heat: plate, wall and door maps colour the surfaces, and the opacity is lighter (the view from behind is covered by unit tests) (1991-3d-heat-surface-maps) | Heat | 1920x1080 | pass | - | 5 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
