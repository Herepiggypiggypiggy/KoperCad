# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 6 run(s): 6 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/target/debug/kopercad`, built 1793721804 (unix time), source at git 4c0b6ac7a7.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| A command window opens docked in the right strip, folds up to its title, unfolds, and Escape closes it (1366x768) (1140-command-window-docks-right-and-folds) | Windows | 1366x768 | pass | - | 4 |
| A command window opens docked in the right strip, folds up to its title, unfolds, and Escape closes it (1920x1080) (1141-command-window-docks-right-and-folds-1920) | Windows | 1920x1080 | pass | - | 4 |
| A settings form (Preferences) opens docked in the right strip, folds, and its close button cancels (1366x768) (1142-form-docks-right-and-cancels) | Windows | 1366x768 | pass | - | 3 |
| A settings form (Preferences) opens docked in the right strip, folds, and its close button cancels (1920x1080) (1143-form-docks-right-and-cancels-1920) | Windows | 1920x1080 | pass | - | 3 |
| Rename terminals opens docked right, folds, and its close button with text typed cancels (1366x768) (1146-terminal-edit-docks-folds-and-cancels) | Windows | 1366x768 | pass | - | 5 |
| The Defects and walk-throughs card docks at the right edge, folds and unfolds (1366x768) (1147-defects-card-docks-and-folds) | Windows | 1366x768 | pass | - | 4 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
