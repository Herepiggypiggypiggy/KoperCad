# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/kopercad/target/debug/kopercad`, built 1793913143 (unix time), source at git aee8bfa667.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Help icons: Parts and Macros carry no explanation text (7432-help-icons-parts-macros) | Parts | 1366x768 | pass | - | 2 |
| Help icons: Parts and Macros carry no explanation text (7432-help-icons-parts-macros) | Parts | 1920x1080 | pass | - | 2 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
