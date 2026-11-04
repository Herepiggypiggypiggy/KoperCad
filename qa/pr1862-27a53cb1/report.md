# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 4 run(s): 4 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793780007 (unix time), source at git 27a53cb144.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Parts window: Import on a Rittal part opens the PARTcommunity hand-off with its steps (1862-rittal-parts-window) | Parts | 1366x768 | pass | - | 2 |
| Parts window: Import on a Rittal part opens the PARTcommunity hand-off with its steps (1862-rittal-parts-window) | Parts | 1920x1080 | pass | - | 2 |
| Cabinet: Maker plate in the Plate setup card, then the drill plan shows factory holes (1862-rittal-plate-drill-plan-1366) | Cabinet | 1366x768 | pass | - | 3 |
| Cabinet: Maker plate in the Plate setup card, then the drill plan shows factory holes (1862-rittal-plate-drill-plan) | Cabinet | 1920x1080 | pass | - | 3 |

## Files kept

Exports the run wrote, copied next to this report:

- `files/1862-rittal-plate-drill-plan-1366-1366x768-Motor_starter__example__+CAB_drill_plan.pdf`
- `files/1862-rittal-plate-drill-plan-1920x1080-Motor_starter__example__+CAB_drill_plan.pdf`

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
