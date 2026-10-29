# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/target/debug/kopercad`, built 1793263844 (unix time), source at git 741f495cc8.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Reports and Commissioning rails (left edge) (7400-reports-commissioning-rails) | Layout | 1366x768 | pass | - | 5 |
| Reports and Commissioning rails (left edge) (7400-reports-commissioning-rails) | Layout | 1920x1080 | pass | - | 5 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
Source tree built and run: code commit ef779a1c3ef0ff8981c7c83f93fed3154cdcd244 (the evidence commit adds only this folder). Screenshots: /mnt/project-files/qa/pr-v3d-ef779a1c/shots/
