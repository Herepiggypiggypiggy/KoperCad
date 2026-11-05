# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 4 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793866074 (unix time), source at git 39c33f3e (tree identical to the build run; run before the commit, on main c28eb10e plus this change).

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Cabinet heat: search for a cooler layout and apply it (14-cooler-layout) | Heat | 1366x768 | pass | F-001, F-002 | 8 |
| Cabinet heat: search for a cooler layout and apply it (14-cooler-layout) | Heat | 1920x1080 | pass | F-003, F-004 | 8 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Heat | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/14-cooler-layout-1366x768-05-finding-1.png |
| F-002 | S4 | Heat | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/14-cooler-layout-1366x768-07-finding-2.png |
| F-003 | S4 | Heat | 1920x1080 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/14-cooler-layout-1920x1080-05-finding-3.png |
| F-004 | S4 | Heat | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/14-cooler-layout-1920x1080-07-finding-4.png |

### F-001 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)
