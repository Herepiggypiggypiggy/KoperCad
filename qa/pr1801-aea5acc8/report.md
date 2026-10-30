# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 19 run(s): 19 passed, 0 failed; 4 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target-panel/debug/kopercad`, built 1793368569 (unix time), source at git aea5acc88f.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Controller import wizard: library table, then a bad pasted table shows what is left out and what to check (1580-controller-import-wizard) | Import | 1366x768 | pass | - | 3 |
| Controller import wizard: library table, then a bad pasted table shows what is left out and what to check (1580-controller-import-wizard) | Import | 1920x1080 | pass | - | 3 |
| Controller panel: a virtual radio drives the receiver of a two-speed hoist (1581-controller-live-radio-hoist) | Live | 1366x768 | pass | - | 5 |
| Controller panel: a virtual radio drives the receiver of a two-speed hoist (1920) (1581b-controller-live-radio-hoist-1920) | Live | 1920x1080 | pass | - | 5 |
| Controller panel opens from Simulate > Controller panel and from the command field (1582-controller-menu-entry) | Live | 1366x768 | pass | - | 2 |
| Controller panel opens from Simulate > Controller panel and from the command field (1582-controller-menu-entry) | Live | 1920x1080 | pass | - | 2 |
| Controller panel and Hoist run: the radio lifts the hook through the drawn circuit (1583-controller-hoist-run) | Live | 1366x768 | pass | - | 4 |
| Controller panel and Hoist run: the radio lifts the hook through the drawn circuit (1920) (1583b-controller-hoist-run-1920) | Live | 1920x1080 | pass | - | 4 |
| Controller panel: a pendant with two-step stacks and a joystick pad with detents (1584-controller-pendant-joystick) | Live | 1366x768 | pass | F-001 | 5 |
| Controller panel: a pendant with two-step stacks and a joystick pad with detents (1920) (1584b-controller-pendant-joystick-1920) | Live | 1920x1080 | pass | - | 4 |
| Controller gamepad: a (fake) pad drives the radio hoist, the dead-man gates it, unplugging stops it (1590-controller-gamepad) | Live | 1366x768 | pass | F-002 | 8 |
| Controller gamepad (1920): a (fake) pad drives the radio hoist, the dead-man gates it, unplugging stops it (1590b-controller-gamepad-1920) | Live | 1920x1080 | pass | - | 7 |
| Controller: several instances as tabs, pop-out panels and shared-target lines (1779-controller-instances-panels) | Live | 1366x768 | pass | - | 4 |
| Controller: several instances as tabs, pop-out panels and shared-target lines (1779-controller-instances-panels) | Live | 1920x1080 | pass | - | 4 |
| Controller add-on: install the pack, open the radio panel, press start and a rocker (5620-controllers-addon) | Add-ons | 1366x768 | pass | - | 5 |
| Controller add-on: the radio panel has tabs, a rotary, step rockers, a key map and a table (5630-controllers-addon-panel-tabs) | Add-ons | 1366x768 | pass | F-003, F-004 | 5 |
| Controller add-on: the radio panel has tabs, a rotary, step rockers, a key map and a table (5630-controllers-addon-panel-tabs) | Add-ons | 1920x1080 | pass | - | 3 |
| Controller card: filter the signals by text, active and unbound (5631-controller-signal-filter) | Live | 1366x768 | pass | - | 3 |
| Controller card: two controllers, the filter lists all of them (5632-controller-all-controllers) | Live | 1366x768 | pass | - | 4 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Live | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1584-controller-pendant-joystick-1366x768-05-finding-1.png |
| F-002 | S4 | Live | 1366x768 | 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1590-controller-gamepad-1366x768-03-finding-2.png |
| F-003 | S4 | Add-ons | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/5630-controllers-addon-panel-tabs-1366x768-03-finding-3.png |
| F-004 | S4 | Add-ons | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/5630-controllers-addon-panel-tabs-1366x768-05-finding-4.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1584-controller-pendant-joystick` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `click_text "+ Add"`
8. `wait 10`
9. `click_text "Wired pendant, 3 motions, 2-step"`
10. `wait 20`
11. `expect "Motions"`
12. `expect "slow"`
13. `expect "fast"`
14. `shot pendant`
15. `check_layout`
16. `click_text "+ Add"`
17. `wait 10`
18. `click_text "Joystick, 2 axes, 3 positions each"`
19. `wait 20`
20. `expect "Joysticks"`
21. `shot joystick-at-rest`
22. `check_layout`
23. `click_text Sticky`
24. `wait 10`
25. `shot sticky-on`
26. `check_layout`
27. `click_text "Key map"`
28. `wait 30`
29. `expect "Hoist up"`
30. `shot key-map`
31. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Live' over 'This controller'

![F-001](shots/1584-controller-pendant-joystick-1366x768-05-finding-1.png)

### F-002 S4 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1590-controller-gamepad` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/radio-hoist.novo`
2. `wait 20`
3. `key shift+l`
4. `wait 20`
5. `click_text "Controller…"`
6. `wait 20`
7. `scroll 1180 500 -700`
8. `wait 10`
9. `click_text "Gamepad"`
10. `wait 20`
11. `scroll 1180 500 -100`
12. `wait 10`
13. `expect "Test gamepad (fake)"`
14. `expect "Connected"`
15. `expect "This controller has no pad map yet."`
16. `shot section-no-map`
17. `click_text "Edit map…"`
18. `wait 20`
19. `expect "Gamepad map"`
20. `click_text "Suggest a map"`
21. `wait 20`
22. `shot map-editor`
23. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '1 Radio control (48 V AC)' over 'Keyboard'; 'Switch page' over 'Sticky'; 'Search' over 'Reset'

![F-002](shots/1590-controller-gamepad-1366x768-03-finding-2.png)

### F-003 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-003](shots/5630-controllers-addon-panel-tabs-1366x768-03-finding-3.png)

### F-004 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-004](shots/5630-controllers-addon-panel-tabs-1366x768-05-finding-4.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
