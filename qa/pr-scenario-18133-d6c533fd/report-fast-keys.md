# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/kopercad/target/debug/kopercad`, built 1793916904 (unix time), source at git d6c533fdd0.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Fast keys card is a pane of the right strip, Live off (18133-fast-keys-pane-without-live) | Learn | 1366x768 | pass | - | 1 |
| Fast keys card is a pane of the right strip, Live off (18133-fast-keys-pane-without-live) | Learn | 1920x1080 | pass | - | 1 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
