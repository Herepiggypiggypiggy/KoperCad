Source at git 5063ab103eec11d2c17b44d523e8b3220cdf11c8 (PR #1969). Scenarios 19690, 19691 at 1366x768 and 1920x1080; the one S4 finding is the docked-windows overlap note (check_layout passed, windows are docked on purpose).

# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 4 run(s): 4 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/tgt/debug/kopercad`, built 1793889680 (unix time), source at git 5063ab103e.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| 3D Heat: Shell setup in Vents and fans, model words visible without hovering (19690-3d-heat-shell-setup) | 3D | 1366x768 | pass | - | 2 |
| 3D Heat: Shell setup in Vents and fans, model words visible without hovering (19690-3d-heat-shell-setup) | 3D | 1920x1080 | pass | - | 2 |
| 3D Heat: Cabinet shell block after a run shows the heat split and face temperatures as model (19691-3d-heat-shell-result) | 3D | 1366x768 | pass | F-001 | 3 |
| 3D Heat: Cabinet shell block after a run shows the heat split and face temperatures as model (19691-3d-heat-shell-result) | 3D | 1920x1080 | pass | - | 2 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | 3D | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | pictures/19691-3d-heat-shell-result-1366x768-02-finding-1.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `19691-3d-heat-shell-result` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `try click_text Cancel`
12. `wait 30`
13. `click_text "Settings for the next run"`
14. `wait 10`
15. `click_text Quick`
16. `wait 10`
17. `click_text Simulate #2`
18. `wait_s 120`
19. `scroll 1150 600 -400`
20. `wait 20`
21. `click_text "Cabinet shell"`
22. `wait 20`
23. `scroll 1150 600 -600`
24. `wait 20`
25. `expect "Through the shell"`
26. `expect "model"`
27. `shot shell-result`
28. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Hide' over 'Wire heat in the ducts'

![F-001](pictures/19691-3d-heat-shell-result-1366x768-02-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
