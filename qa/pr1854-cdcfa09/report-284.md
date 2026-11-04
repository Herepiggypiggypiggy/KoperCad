# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/ct/debug/kopercad`, built 1793776085 (unix time), source at git cdcfa09b3a.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Lesson walk by real clicks: Will the door close? (Apply, already applied, ring inside the Inspector) (284-learn-door-real-clicks) | Learn | 1366x768 | pass | - | 8 |
| Lesson walk by real clicks: Will the door close? (Apply, already applied, ring inside the Inspector) (284-learn-door-real-clicks) | Learn | 1920x1080 | pass | - | 8 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
