# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 1 run(s): 1 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/target/debug/kopercad`, built 1793267525 (unix time), source at git 59111adce1.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| A wire under way: the tool rail stays put and Select still works (736-wire-with-the-rail) | Schematic | 1366x768 | pass | - | 4 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
