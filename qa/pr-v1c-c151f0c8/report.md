# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 7 run(s): 7 passed, 0 failed; 2 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793281753 (unix time), source at git c151f0c8b3.

App: 1920.0x1080.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Drag the Inspector's title bar to the left edge of the canvas to dock it next to the pinned Pages column, drag the slot wider and double click its edge to reset (1920x1080) (1122-dock-by-dragging-1920) | Windows | 1920x1080 | pass | - | 3 |
| Dock the Inspector with its title-bar buttons, move it between edges and float it again (1920x1080) (1123-dock-inspector-1920) | Windows | 1920x1080 | pass | - | 5 |
| Two docked panels share an edge: drag the divider between them, double click it to make them equal (1366x768) (1132-docked-panels-share-an-edge) | Windows | 1366x768 | pass | F-001, F-002 | 5 |
| Two docked panels share an edge: drag the divider between them, double click it to make them equal (1920x1080) (1133-docked-panels-share-an-edge-1920) | Windows | 1920x1080 | pass | - | 3 |
| The pinned Pages column moves to the right side of the window and back with the arrow in its header (1920x1080) (1134-pages-column-to-the-right-1920) | Windows | 1920x1080 | pass | - | 3 |
| Pages docks as a pane of the left strip with the button in its header, moves to the right strip and floats back to the column (1366x768) (1135-pages-docks-as-a-pane-1366) | Windows | 1366x768 | pass | - | 3 |
| Pages docks as a pane of the left strip with the button in its header, moves to the right strip and floats back to the column (1920x1080) (1135-pages-docks-as-a-pane-1920) | Windows | 1920x1080 | pass | - | 3 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Windows | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1132-docked-panels-share-an-edge-1366x768-02-finding-1.png |
| F-002 | S4 | Windows | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1132-docked-panels-share-an-edge-1366x768-05-finding-2.png |

### F-001 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1132-docked-panels-share-an-edge` at 1366x768. Kind: layout-maybe.

Steps:

1. `click_text "Open example"`
2. `wait 30`
3. `click 353 228`
4. `wait 40`
5. `click 1309 112`
6. `wait 30`
7. `key ctrl+k`
8. `wait 4`
9. `text review notes`
10. `wait 6`
11. `key Enter`
12. `wait 30`
13. `click 1019 112`
14. `wait 30`
15. `expect "No notes yet"`
16. `shot two-panes`
17. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '9' over 'All 0'; '9' over ' Note'

![F-001](shots/1132-docked-panels-share-an-edge-1366x768-02-finding-1.png)

### F-002 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1132-docked-panels-share-an-edge` at 1366x768. Kind: layout-maybe.

Steps:

1. `click_text "Open example"`
2. `wait 30`
3. `click 353 228`
4. `wait 40`
5. `click 1309 112`
6. `wait 30`
7. `key ctrl+k`
8. `wait 4`
9. `text review notes`
10. `wait 6`
11. `key Enter`
12. `wait 30`
13. `click 1019 112`
14. `wait 30`
15. `expect "No notes yet"`
16. `shot two-panes`
17. `check_layout`
18. `drag 1200 391 1200 520`
19. `wait 20`
20. `shot divider-moved`
21. `check_layout`
22. `double_click 1200 520`
23. `wait 20`
24. `shot divider-reset`
25. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '9' over 'All 0'; '9' over ' Note'

![F-002](shots/1132-docked-panels-share-an-edge-1366x768-05-finding-2.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
