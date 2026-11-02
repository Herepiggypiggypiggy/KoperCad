# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 19 run(s): 17 passed, 2 failed; 8 finding(s) (7 blocking, S1 to S3).

Program: `/tmp/ktarget/debug/kopercad`, built 1793623123 (unix time), source at git eb9330f0cb.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Cabinet: box select three devices, drag them together, undo and redo restore the whole group (18130-cabinet-group-drag) | Cabinet | 1366x768 | pass | - | 5 |
| Cabinet: box select three devices, drag them together, undo and redo restore the whole group (18130b-cabinet-group-drag-1920) | Cabinet | 1920x1080 | pass | F-001 | 6 |
| Cabinet: Shift-click, the Align row (2 and 3 devices), Ctrl+A and the arrow keys (18131-cabinet-group-align-nudge) | Cabinet | 1366x768 | pass | - | 7 |
| Cabinet: Shift-click, the Align row (2 and 3 devices), Ctrl+A and the arrow keys (18131b-cabinet-group-align-nudge-1920) | Cabinet | 1920x1080 | pass | - | 7 |
| Cabinet: a device dragged near its rail clips onto it by its centre, one dragged away leaves it (79-cabinet-rail-snap) | Cabinet | 1366x768 | fail | F-002, F-003, F-004, F-005 | 9 |
| Cabinet rail: rows, expand, panels zone (770-cabinet-rail) | Layout | 1366x768 | pass | - | 2 |
| Cabinet rail: rows, expand, panels zone (770-cabinet-rail) | Layout | 1920x1080 | pass | - | 2 |
| Journey: example project to placed, routed and cut-listed cabinet (13-journey-cabinet) | Journeys | 1366x768 | pass | - | 8 |
| Journey: example project to placed, routed and cut-listed cabinet (13-journey-cabinet) | Journeys | 1920x1080 | pass | - | 8 |
| Cabinet heat: the Flow menu draws arrows, streamlines or a speed map on one m/s key (1640-cabinet-flow-modes) | Heat | 1366x768 | pass | - | 3 |
| Cabinet heat: the Flow menu draws arrows, streamlines or a speed map on one m/s key (1640-cabinet-flow-modes) | Heat | 1920x1080 | pass | - | 3 |
| Cabinet: field, door and plate (Mounted row, Size enclosure note, Issues, Place all) (1700-cabinet-sites) | Cabinet | 1366x768 | fail | F-006, F-007, F-008 | 5 |
| Cabinet: field, door and plate (Mounted row, Size enclosure note, Issues, Place all) (1700-cabinet-sites) | Cabinet | 1920x1080 | pass | - | 9 |
| Cabinet: a new plate size resizes the rails and ducts (plate follow) (1711-cabinet-plate-resize-follows) | Cabinet | 1366x768 | pass | - | 6 |
| Cabinet: a new plate size resizes the rails and ducts (plate follow) (1711-cabinet-plate-resize-follows) | Cabinet | 1920x1080 | pass | - | 6 |
| Cabinet: the problems pill opens the list of what is wrong (15-cabinet-problem-pill) | Cabinet | 1366x768 | pass | - | 2 |
| Cabinet: the problems pill opens the list of what is wrong (15-cabinet-problem-pill) | Cabinet | 1920x1080 | pass | - | 2 |
| Cabinet toolbar: a project with two panels fits at 1366 wide (58-cabinet-toolbar-two-panels-1366) | Cabinet | 1366x768 | pass | - | 1 |
| Cabinet toolbar: a project with two panels fits at 1366 wide (58-cabinet-toolbar-two-panels-1366) | Cabinet | 1920x1080 | pass | - | 1 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Cabinet | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18130b-cabinet-group-drag-1920-1920x1080-04-finding-1.png |
| F-002 | S3 | Cabinet | 1366x768 | `55, 210 mm` not on screen | shots/79-cabinet-rail-snap-1366x768-03-finding-2.png |
| F-003 | S3 | Cabinet | 1366x768 | `Mounted on` not on screen | shots/79-cabinet-rail-snap-1366x768-04-finding-3.png |
| F-004 | S3 | Cabinet | 1366x768 | `210 mm` not on screen | shots/79-cabinet-rail-snap-1366x768-06-finding-4.png |
| F-005 | S3 | Cabinet | 1366x768 | `Mounted on` not on screen | shots/79-cabinet-rail-snap-1366x768-07-finding-5.png |
| F-006 | S3 | Cabinet | 1366x768 | `Mounted` not on screen | shots/1700-cabinet-sites-1366x768-03-finding-6.png |
| F-007 | S3 | Cabinet | 1366x768 | `In the field` not on screen | shots/1700-cabinet-sites-1366x768-04-finding-7.png |
| F-008 | S3 | Cabinet | 1366x768 | step failed: click_text "In the field" | shots/1700-cabinet-sites-1366x768-05-finding-8.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18130b-cabinet-group-drag-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cabinet-lesson.novo`
2. `wait 30`
3. `click_text Cabinet #1`
4. `wait 30`
5. `move 700 700`
6. `shot before`
7. `check_layout`
8. `drag 1090 160 905 262`
9. `wait 15`
10. `move 700 700`
11. `wait 5`
12. `expect "Align"`
13. `expect "3 selected"`
14. `expect "55, 10 mm"`
15. `shot box-selected`
16. `drag 1009 207 1250 590`
17. `wait 20`
18. `move 700 700`
19. `wait 5`
20. `expect_not "55, 10 mm"`
21. `shot group-dragged`
22. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Left' over 'In the cabinet'

![F-001](shots/18130b-cabinet-group-drag-1920-1920x1080-04-finding-1.png)

### F-002 S3 `55, 210 mm` not on screen

Scenario `79-cabinet-rail-snap` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-lesson.novo`
2. `wait 30`
3. `click_text Cabinet #1`
4. `wait 30`
5. `shot plate`
6. `check_blank plate`
7. `check_layout`
8. `click 600 329`
9. `wait 10`
10. `expect "55, 210 mm"`

- Expected: a control or text containing `55, 210 mm`
- Saw: not found in the widget tree

![F-002](shots/79-cabinet-rail-snap-1366x768-03-finding-2.png)

### F-003 S3 `Mounted on` not on screen

Scenario `79-cabinet-rail-snap` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-lesson.novo`
2. `wait 30`
3. `click_text Cabinet #1`
4. `wait 30`
5. `shot plate`
6. `check_blank plate`
7. `check_layout`
8. `click 600 329`
9. `wait 10`
10. `expect "55, 210 mm"`
11. `click 1102 426`
12. `wait 10`
13. `expect "Mounted on"`

- Expected: a control or text containing `Mounted on`
- Saw: not found in the widget tree

![F-003](shots/79-cabinet-rail-snap-1366x768-04-finding-3.png)

### F-004 S3 `210 mm` not on screen

Scenario `79-cabinet-rail-snap` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-lesson.novo`
2. `wait 30`
3. `click_text Cabinet #1`
4. `wait 30`
5. `shot plate`
6. `check_blank plate`
7. `check_layout`
8. `click 600 329`
9. `wait 10`
10. `expect "55, 210 mm"`
11. `click 1102 426`
12. `wait 10`
13. `expect "Mounted on"`
14. `shot k1-selected`
15. `drag 600 329 616 334`
16. `wait 20`
17. `expect "210 mm"`

- Expected: a control or text containing `210 mm`
- Saw: not found in the widget tree

![F-004](shots/79-cabinet-rail-snap-1366x768-06-finding-4.png)

### F-005 S3 `Mounted on` not on screen

Scenario `79-cabinet-rail-snap` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-lesson.novo`
2. `wait 30`
3. `click_text Cabinet #1`
4. `wait 30`
5. `shot plate`
6. `check_blank plate`
7. `check_layout`
8. `click 600 329`
9. `wait 10`
10. `expect "55, 210 mm"`
11. `click 1102 426`
12. `wait 10`
13. `expect "Mounted on"`
14. `shot k1-selected`
15. `drag 600 329 616 334`
16. `wait 20`
17. `expect "210 mm"`
18. `expect_not "55, 210 mm"`
19. `expect "Mounted on"`

- Expected: a control or text containing `Mounted on`
- Saw: not found in the widget tree

![F-005](shots/79-cabinet-rail-snap-1366x768-07-finding-5.png)

### F-006 S3 `Mounted` not on screen

Scenario `1700-cabinet-sites` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-sites.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 20`
5. `shot cabinet-before`
6. `click 969 192`
7. `try click 1470 218`
8. `wait 10`
9. `shot inspector-mounted`
10. `expect "Mounted"`

- Expected: a control or text containing `Mounted`
- Saw: not found in the widget tree

![F-006](shots/1700-cabinet-sites-1366x768-03-finding-6.png)

### F-007 S3 `In the field` not on screen

Scenario `1700-cabinet-sites` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-sites.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 20`
5. `shot cabinet-before`
6. `click 969 192`
7. `try click 1470 218`
8. `wait 10`
9. `shot inspector-mounted`
10. `expect "Mounted"`
11. `expect "In the field"`

- Expected: a control or text containing `In the field`
- Saw: not found in the widget tree

![F-007](shots/1700-cabinet-sites-1366x768-04-finding-7.png)

### F-008 S3 step failed: click_text "In the field"

Scenario `1700-cabinet-sites` at 1366x768. Kind: step.

Steps:

1. `open qa/fixtures/cabinet-sites.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 20`
5. `shot cabinet-before`
6. `click 969 192`
7. `try click 1470 218`
8. `wait 10`
9. `shot inspector-mounted`
10. `expect "Mounted"`
11. `expect "In the field"`
12. `click_text "In the field"`

- Expected: the step runs
- Saw: no enabled widget labelled `In the field` (match 1)

![F-008](shots/1700-cabinet-sites-1366x768-05-finding-8.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
