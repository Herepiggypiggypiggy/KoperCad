# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 17 run(s): 17 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793871767 (unix time), source at git fec08f150f.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| 3D Heat vents: air in and air out balance (solver fluxes) with a fan and a grille (1814-3d-vents-air-in-equals-air-out) | 3D | 1366x768 | pass | - | 3 |
| 3D Heat vents: air in and air out balance (solver fluxes) with a fan and a grille (1814-3d-vents-air-in-equals-air-out) | 3D | 1920x1080 | pass | - | 3 |
| Top bar: Help button, What's new as an icon, workspace tooltip (724-top-bar-help) | Layout | 1366x768 | pass | - | 3 |
| Top bar: Help button, What's new as an icon, workspace tooltip (724-top-bar-help) | Layout | 1920x1080 | pass | - | 3 |
| Help popups: EMC check, Bench output and Ask Claude windows (7392-help-popups-emc-bench) | Windows | 1366x768 | pass | - | 4 |
| Help popups: EMC check, Bench output and Ask Claude windows (7392-help-popups-emc-bench) | Windows | 1920x1080 | pass | - | 4 |
| Help icons: the 3D Heat strip, Heat panel and Vents window carry no explanation text (7430-help-icons-3d-heat) | 3D | 1366x768 | pass | F-001 | 5 |
| Help icons: the 3D Heat strip, Heat panel and Vents window carry no explanation text (7430-help-icons-3d-heat) | 3D | 1920x1080 | pass | - | 4 |
| Help icons: a long ? popup scrolls inside the window at 1366x768 (7433-help-popup-scroll-1366x768) | 3D | 1366x768 | pass | - | 3 |
| Help icons: a long ? popup scrolls inside the window at 1920x1080 (7434-help-popup-scroll-1920x1080) | 3D | 1920x1080 | pass | - | 3 |
| Help icons: a short ? popup keeps its natural size, no gap and no scrollbar at 1366x768 (7435-help-popup-short-1366x768) | 3D | 1366x768 | pass | - | 1 |
| Cabinet Routes panel: warnings are count chips with a fix button (9510-routes-count-chips) | Cabinet | 1366x768 | pass | - | 2 |
| Cabinet Routes panel: warnings are count chips with a fix button (9510-routes-count-chips) | Cabinet | 1920x1080 | pass | - | 2 |
| A failed import is a red triangle in the top bar with a popover; a success clears it (1366x768) (9522-job-flag-failed-import-1366x768) | Files | 1366x768 | pass | - | 6 |
| A failed import is a red triangle in the top bar with a popover; a success clears it (1920x1080) (9523-job-flag-failed-import-1920x1080) | Files | 1920x1080 | pass | - | 6 |
| Bench: the readiness sentences are yellow triangle chips with the sentence in a popover (1366x768) (9524-bench-estimate-chip-1366x768) | Reports | 1366x768 | pass | - | 5 |
| Bench: the readiness sentences are yellow triangle chips with the sentence in a popover (1920x1080) (9525-bench-estimate-chip-1920x1080) | Reports | 1920x1080 | pass | - | 5 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | 3D | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/7430-help-icons-3d-heat-1366x768-02-finding-1.png |

### F-001 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `7430-help-icons-3d-heat` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cabinet-cooling-advice.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 60`
16. `try click_text Cancel`
17. `wait 20`
18. `click_text "Settings for the next run"`
19. `wait 10`
20. `try click_text Quick`
21. `wait 10`
22. `click_text Simulate #2`
23. `wait_s 200`
24. `shot heat-clean`
25. `check_text 40`
26. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Warning: Device off its rail's middle (a...' over 'Add the filter fan and a grille to the c...'; 'Warning: Device off its rail's middle' over 'Add the filter fan and a grille to the c...'

![F-001](shots/7430-help-icons-3d-heat-1366x768-02-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
