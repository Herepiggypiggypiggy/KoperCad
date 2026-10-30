# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 17 run(s): 11 passed, 6 failed; 12 finding(s) (10 blocking, S1 to S3).

Program: `/tmp/kc-target-panel/debug/kopercad`, built 1793366548 (unix time), source at git c71a3f5f97.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Controller import wizard: library table, then a bad pasted table shows what is left out and what to check (1580-controller-import-wizard) | Import | 1366x768 | pass | - | 3 |
| Controller import wizard: library table, then a bad pasted table shows what is left out and what to check (1580-controller-import-wizard) | Import | 1920x1080 | pass | - | 3 |
| Controller panel: a virtual radio drives the receiver of a two-speed hoist (1581-controller-live-radio-hoist) | Live | 1366x768 | fail | F-001, F-002, F-003 | 8 |
| Controller panel: a virtual radio drives the receiver of a two-speed hoist (1920) (1581b-controller-live-radio-hoist-1920) | Live | 1920x1080 | fail | F-004, F-005, F-006 | 8 |
| Controller panel opens from Simulate > Controller panel and from the command field (1582-controller-menu-entry) | Live | 1366x768 | pass | - | 2 |
| Controller panel opens from Simulate > Controller panel and from the command field (1582-controller-menu-entry) | Live | 1920x1080 | pass | - | 2 |
| Controller panel and Hoist run: the radio lifts the hook through the drawn circuit (1583-controller-hoist-run) | Live | 1366x768 | fail | F-007 | 5 |
| Controller panel and Hoist run: the radio lifts the hook through the drawn circuit (1920) (1583b-controller-hoist-run-1920) | Live | 1920x1080 | fail | F-008 | 5 |
| Controller panel: a pendant with two-step stacks and a joystick pad with detents (1584-controller-pendant-joystick) | Live | 1366x768 | fail | F-009 | 1 |
| Controller panel: a pendant with two-step stacks and a joystick pad with detents (1920) (1584b-controller-pendant-joystick-1920) | Live | 1920x1080 | fail | F-010 | 1 |
| Controller: several instances as tabs, pop-out panels and shared-target lines (1779-controller-instances-panels) | Live | 1366x768 | pass | - | 4 |
| Controller: several instances as tabs, pop-out panels and shared-target lines (1779-controller-instances-panels) | Live | 1920x1080 | pass | - | 4 |
| Controller add-on: install the pack, open the radio panel, press start and a rocker (5620-controllers-addon) | Add-ons | 1366x768 | pass | - | 5 |
| Controller add-on: the radio panel has tabs, a rotary, step rockers, a key map and a table (5630-controllers-addon-panel-tabs) | Add-ons | 1366x768 | pass | F-011, F-012 | 5 |
| Controller add-on: the radio panel has tabs, a rotary, step rockers, a key map and a table (5630-controllers-addon-panel-tabs) | Add-ons | 1920x1080 | pass | - | 3 |
| Controller card: filter the signals by text, active and unbound (5631-controller-signal-filter) | Live | 1366x768 | pass | - | 3 |
| Controller card: two controllers, the filter lists all of them (5632-controller-all-controllers) | Live | 1366x768 | pass | - | 4 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S3 | Live | 1366x768 | `Signals (2 of 15 high)` not on screen | shots/1581-controller-live-radio-hoist-1366x768-02-finding-1.png |
| F-002 | S3 | Live | 1366x768 | `Signals (4 of 15 high)` not on screen | shots/1581-controller-live-radio-hoist-1366x768-04-finding-2.png |
| F-003 | S3 | Live | 1366x768 | `Signals (2 of 15 high)` not on screen | shots/1581-controller-live-radio-hoist-1366x768-06-finding-3.png |
| F-004 | S3 | Live | 1920x1080 | `Signals (2 of 15 high)` not on screen | shots/1581b-controller-live-radio-hoist-1920-1920x1080-02-finding-4.png |
| F-005 | S3 | Live | 1920x1080 | `Signals (4 of 15 high)` not on screen | shots/1581b-controller-live-radio-hoist-1920-1920x1080-04-finding-5.png |
| F-006 | S3 | Live | 1920x1080 | `Signals (2 of 15 high)` not on screen | shots/1581b-controller-live-radio-hoist-1920-1920x1080-06-finding-6.png |
| F-007 | S3 | Live | 1366x768 | `brake open` not on screen | shots/1583-controller-hoist-run-1366x768-02-finding-7.png |
| F-008 | S3 | Live | 1920x1080 | `brake open` not on screen | shots/1583b-controller-hoist-run-1920-1920x1080-02-finding-8.png |
| F-009 | S3 | Live | 1366x768 | step failed: click_text "Wired pendant, 3 motions, 2-step" | shots/1584-controller-pendant-joystick-1366x768-01-finding-9.png |
| F-010 | S3 | Live | 1920x1080 | step failed: click_text "Wired pendant, 3 motions, 2-step" | shots/1584b-controller-pendant-joystick-1920-1920x1080-01-finding-10.png |
| F-011 | S4 | Add-ons | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/5630-controllers-addon-panel-tabs-1366x768-03-finding-11.png |
| F-012 | S4 | Add-ons | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/5630-controllers-addon-panel-tabs-1366x768-05-finding-12.png |

### F-001 S3 `Signals (2 of 15 high)` not on screen

Scenario `1581-controller-live-radio-hoist` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `expect "Simulation of wiring and logic"`
8. `expect "Generic controller, not a specific product"`
9. `expect "Signals (1 of 15 high)"`
10. `expect "9 signals drive nothing"`
11. `shot panel-at-rest`
12. `check_layout`
13. `click 1052 315`
14. `wait 10`
15. `expect "Signals (2 of 15 high)"`

- Expected: a control or text containing `Signals (2 of 15 high)`
- Saw: not found in the widget tree

![F-001](shots/1581-controller-live-radio-hoist-1366x768-02-finding-1.png)

### F-002 S3 `Signals (4 of 15 high)` not on screen

Scenario `1581-controller-live-radio-hoist` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `expect "Simulation of wiring and logic"`
8. `expect "Generic controller, not a specific product"`
9. `expect "Signals (1 of 15 high)"`
10. `expect "9 signals drive nothing"`
11. `shot panel-at-rest`
12. `check_layout`
13. `click 1052 315`
14. `wait 10`
15. `expect "Signals (2 of 15 high)"`
16. `shot main-on`
17. `mouse_down 1081 482`
18. `wait 10`
19. `expect "Signals (4 of 15 high)"`

- Expected: a control or text containing `Signals (4 of 15 high)`
- Saw: not found in the widget tree

![F-002](shots/1581-controller-live-radio-hoist-1366x768-04-finding-2.png)

### F-003 S3 `Signals (2 of 15 high)` not on screen

Scenario `1581-controller-live-radio-hoist` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `expect "Simulation of wiring and logic"`
8. `expect "Generic controller, not a specific product"`
9. `expect "Signals (1 of 15 high)"`
10. `expect "9 signals drive nothing"`
11. `shot panel-at-rest`
12. `check_layout`
13. `click 1052 315`
14. `wait 10`
15. `expect "Signals (2 of 15 high)"`
16. `shot main-on`
17. `mouse_down 1081 482`
18. `wait 10`
19. `expect "Signals (4 of 15 high)"`
20. `shot hoist-up-2`
21. `check_layout`
22. `mouse_up 1081 482`
23. `wait 10`
24. `expect "Signals (2 of 15 high)"`

- Expected: a control or text containing `Signals (2 of 15 high)`
- Saw: not found in the widget tree

![F-003](shots/1581-controller-live-radio-hoist-1366x768-06-finding-3.png)

### F-004 S3 `Signals (2 of 15 high)` not on screen

Scenario `1581b-controller-live-radio-hoist-1920` at 1920x1080. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `expect "Simulation of wiring and logic"`
8. `expect "Generic controller, not a specific product"`
9. `expect "Signals (1 of 15 high)"`
10. `expect "9 signals drive nothing"`
11. `shot panel-at-rest`
12. `check_layout`
13. `click 1607 315`
14. `wait 10`
15. `expect "Signals (2 of 15 high)"`

- Expected: a control or text containing `Signals (2 of 15 high)`
- Saw: not found in the widget tree

![F-004](shots/1581b-controller-live-radio-hoist-1920-1920x1080-02-finding-4.png)

### F-005 S3 `Signals (4 of 15 high)` not on screen

Scenario `1581b-controller-live-radio-hoist-1920` at 1920x1080. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `expect "Simulation of wiring and logic"`
8. `expect "Generic controller, not a specific product"`
9. `expect "Signals (1 of 15 high)"`
10. `expect "9 signals drive nothing"`
11. `shot panel-at-rest`
12. `check_layout`
13. `click 1607 315`
14. `wait 10`
15. `expect "Signals (2 of 15 high)"`
16. `shot main-on`
17. `mouse_down 1640 482`
18. `wait 10`
19. `expect "Signals (4 of 15 high)"`

- Expected: a control or text containing `Signals (4 of 15 high)`
- Saw: not found in the widget tree

![F-005](shots/1581b-controller-live-radio-hoist-1920-1920x1080-04-finding-5.png)

### F-006 S3 `Signals (2 of 15 high)` not on screen

Scenario `1581b-controller-live-radio-hoist-1920` at 1920x1080. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `expect "Simulation of wiring and logic"`
8. `expect "Generic controller, not a specific product"`
9. `expect "Signals (1 of 15 high)"`
10. `expect "9 signals drive nothing"`
11. `shot panel-at-rest`
12. `check_layout`
13. `click 1607 315`
14. `wait 10`
15. `expect "Signals (2 of 15 high)"`
16. `shot main-on`
17. `mouse_down 1640 482`
18. `wait 10`
19. `expect "Signals (4 of 15 high)"`
20. `shot hoist-up-2`
21. `check_layout`
22. `mouse_up 1640 482`
23. `wait 10`
24. `expect "Signals (2 of 15 high)"`

- Expected: a control or text containing `Signals (2 of 15 high)`
- Saw: not found in the widget tree

![F-006](shots/1581b-controller-live-radio-hoist-1920-1920x1080-06-finding-6.png)

### F-007 S3 `brake open` not on screen

Scenario `1583-controller-hoist-run` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 3`
5. `text Simulate hoist run`
6. `wait 4`
7. `key Enter`
8. `wait 20`
9. `click_text "Switch on Live"`
10. `wait 20`
11. `click_text "Start"`
12. `wait 20`
13. `drag 1000 82 480 82`
14. `wait 10`
15. `click_text "Controller…"`
16. `wait 20`
17. `expect "Hoists in this project: -M1"`
18. `expect "0 rpm · off · brake closed"`
19. `shot before-main`
20. `click 1052 315`
21. `wait 10`
22. `mouse_down 1081 452`
23. `wait 120`
24. `expect "fine"`
25. `expect "brake open"`

- Expected: a control or text containing `brake open`
- Saw: not found in the widget tree

![F-007](shots/1583-controller-hoist-run-1366x768-02-finding-7.png)

### F-008 S3 `brake open` not on screen

Scenario `1583b-controller-hoist-run-1920` at 1920x1080. Kind: expect.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 3`
5. `text Simulate hoist run`
6. `wait 4`
7. `key Enter`
8. `wait 20`
9. `click_text "Switch on Live"`
10. `wait 20`
11. `click_text "Start"`
12. `wait 20`
13. `drag 1554 82 1034 82`
14. `wait 10`
15. `click_text "Controller…"`
16. `wait 20`
17. `expect "Hoists in this project: -M1"`
18. `expect "0 rpm · off · brake closed"`
19. `shot before-main`
20. `click 1607 315`
21. `wait 10`
22. `mouse_down 1640 452`
23. `wait 120`
24. `expect "fine"`
25. `expect "brake open"`

- Expected: a control or text containing `brake open`
- Saw: not found in the widget tree

![F-008](shots/1583b-controller-hoist-run-1920-1920x1080-02-finding-8.png)

### F-009 S3 step failed: click_text "Wired pendant, 3 motions, 2-step"

Scenario `1584-controller-pendant-joystick` at 1366x768. Kind: step.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `click_text Add`
8. `wait 10`
9. `click_text "Wired pendant, 3 motions, 2-step"`

- Expected: the step runs
- Saw: no enabled widget labelled `Wired pendant, 3 motions, 2-step` (match 1)

![F-009](shots/1584-controller-pendant-joystick-1366x768-01-finding-9.png)

### F-010 S3 step failed: click_text "Wired pendant, 3 motions, 2-step"

Scenario `1584b-controller-pendant-joystick-1920` at 1920x1080. Kind: step.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `click_text Add`
8. `wait 10`
9. `click_text "Wired pendant, 3 motions, 2-step"`

- Expected: the step runs
- Saw: no enabled widget labelled `Wired pendant, 3 motions, 2-step` (match 1)

![F-010](shots/1584b-controller-pendant-joystick-1920-1920x1080-01-finding-10.png)

### F-011 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `5630-controllers-addon-panel-tabs` at 1366x768. Kind: layout-maybe.

Steps:

1. `click_text "Open example"`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text add-ons`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `answer_dialog target/addons/controllers-0.2.0.kcaddon`
10. `click_text "Install from file..."`
11. `wait 10`
12. `click_text "I trust this file"`
13. `wait 4`
14. `click_text "Install" Button`
15. `wait 12`
16. `key Escape`
17. `wait 4`
18. `key ctrl+k`
19. `wait 6`
20. `text radio panel`
21. `wait 8`
22. `key Enter`
23. `wait 10`
24. `shot motions-tab`
25. `expect "Hoist select"`
26. `expect "Mast hoist"`
27. `check_layout`
28. `click_text "Keys"`
29. `wait 6`
30. `shot keys-tab`
31. `expect "Main latch"`
32. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Hide' over 'Close window'

![F-011](shots/5630-controllers-addon-panel-tabs-1366x768-03-finding-11.png)

### F-012 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `5630-controllers-addon-panel-tabs` at 1366x768. Kind: layout-maybe.

Steps:

1. `click_text "Open example"`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text add-ons`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `answer_dialog target/addons/controllers-0.2.0.kcaddon`
10. `click_text "Install from file..."`
11. `wait 10`
12. `click_text "I trust this file"`
13. `wait 4`
14. `click_text "Install" Button`
15. `wait 12`
16. `key Escape`
17. `wait 4`
18. `key ctrl+k`
19. `wait 6`
20. `text radio panel`
21. `wait 8`
22. `key Enter`
23. `wait 10`
24. `shot motions-tab`
25. `expect "Hoist select"`
26. `expect "Mast hoist"`
27. `check_layout`
28. `click_text "Keys"`
29. `wait 6`
30. `shot keys-tab`
31. `expect "Main latch"`
32. `check_layout`
33. `click_text "Contacts"`
34. `wait 6`
35. `shot contacts-tab`
36. `expect "Contacts of a 2-step rocker"`
37. `expect "closed"`
38. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Hide' over 'Close window'

![F-012](shots/5630-controllers-addon-panel-tabs-1366x768-05-finding-12.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
