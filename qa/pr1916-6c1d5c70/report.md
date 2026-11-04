# PR 1916 evidence (source at git 6c1d5c70754bcc4362d2a1126e6139c8b137f103)

Scenarios 9530 (1366x768) and 9531 (1920x1080): 2 runs, 0 failed, 0 findings. Pictures: /mnt/project-files/qa/t13-6c1d5c70/

# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/claude-0/target/debug/kopercad`, built 1793796306 (unix time), source at git 6c1d5c7075.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Commissioning: a Blocking punch item draws a red triangle with a popover (9530-commissioning-blocking-punch-triangle) | Commissioning | 1366x768 | pass | - | 3 |
| Commissioning: a Blocking punch item draws a red triangle with a popover (9531-commissioning-blocking-punch-triangle-1920) | Commissioning | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
