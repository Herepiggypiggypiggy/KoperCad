# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/dev/shm/kc-target-gamepad/debug/kopercad`, built 1793362585 (unix time), source at git 4468c86c3f.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Controller gamepad: a (fake) pad drives the radio hoist, the dead-man gates it, unplugging stops it (1590-controller-gamepad) | Live | 1366x768 | pass | - | 7 |
| Controller gamepad (1920): a (fake) pad drives the radio hoist, the dead-man gates it, unplugging stops it (1590b-controller-gamepad-1920) | Live | 1920x1080 | pass | - | 7 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
