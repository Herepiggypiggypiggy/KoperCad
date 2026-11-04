# PR 1916 evidence (source at git 4e17fab18dd0662ac64da84e46f2343f070271e0)

Pictures: /mnt/project-files/qa/pr1916-4e17fab1/shots/

# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 10 run(s): 10 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/claude-0/target/debug/kopercad`, built 1793821623 (unix time), source at git 4e17fab18d.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Commissioning side panel shows the Project and This page counters whole (634-commissioning-panel-shows-its-counters) | Commissioning | 1366x768 | pass | - | 1 |
| Commissioning side panel shows the Project and This page counters whole (634-commissioning-panel-shows-its-counters) | Commissioning | 1920x1080 | pass | - | 1 |
| Reports and Commissioning rails (left edge) (7400-reports-commissioning-rails) | Layout | 1366x768 | pass | - | 5 |
| Reports and Commissioning rails (left edge) (7400-reports-commissioning-rails) | Layout | 1920x1080 | pass | - | 5 |
| Commissioning: a Blocking punch item draws a red triangle with a popover (9530-commissioning-blocking-punch-triangle) | Commissioning | 1366x768 | pass | - | 3 |
| Commissioning: a Blocking punch item draws a red triangle with a popover (9531-commissioning-blocking-punch-triangle-1920) | Commissioning | 1920x1080 | pass | - | 3 |
| Commissioning: a failed field results import is a job flag with Retry and Dismiss (9532-commissioning-failed-import-job-flag) | Commissioning | 1366x768 | pass | - | 3 |
| Commissioning: a failed field results import is a job flag with Retry and Dismiss (9532-commissioning-failed-import-job-flag) | Commissioning | 1920x1080 | pass | - | 3 |
| Commissioning: a failed check is a red triangle on its row and wire, and an open redline a yellow one (9533-commissioning-failed-check-and-redline) | Commissioning | 1366x768 | pass | - | 3 |
| Commissioning: a failed check is a red triangle on its row and wire, and an open redline a yellow one (9533-commissioning-failed-check-and-redline) | Commissioning | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
