# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 0 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/ktarget/debug/kopercad`, built 1793624305 (unix time), source at git eb9330f0cb.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Cabinet: Shift-click two devices, drag one, both move; undo puts both back (18132-cabinet-shift-click-then-drag) | Cabinet | 1366x768 | pass | - | 3 |
| Cabinet: Shift-click two devices, drag one, both move; undo puts both back (18132b-cabinet-shift-click-then-drag-1920) | Cabinet | 1920x1080 | pass | - | 3 |

## Findings

None.


Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
