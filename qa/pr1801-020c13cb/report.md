# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 16 run(s): 16 passed, 0 failed; 2 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target-panel/debug/kopercad`, built 1793362695 (unix time), source at git f976c8e4ab.

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
| Controller panel: a pendant with two-step stacks and a joystick pad with detents (1584-controller-pendant-joystick) | Live | 1366x768 | pass | - | 4 |
| Controller panel: a pendant with two-step stacks and a joystick pad with detents (1920) (1584b-controller-pendant-joystick-1920) | Live | 1920x1080 | pass | - | 4 |
| Controller add-on: install the pack, open the radio panel, press start and a rocker (5620-controllers-addon) | Add-ons | 1366x768 | pass | - | 5 |
| Controller add-on: the radio panel has tabs, a rotary, step rockers, a key map and a table (5630-controllers-addon-panel-tabs) | Add-ons | 1366x768 | pass | F-001, F-002 | 5 |
| Controller add-on: the radio panel has tabs, a rotary, step rockers, a key map and a table (5630-controllers-addon-panel-tabs) | Add-ons | 1920x1080 | pass | - | 3 |
| Controller card: filter the signals by text, active and unbound (5631-controller-signal-filter) | Live | 1366x768 | pass | - | 3 |
| Controller card: two controllers, the filter lists all of them (5632-controller-all-controllers) | Live | 1366x768 | pass | - | 3 |
| Controller card: pop a controller out into its own window (5633-controller-pop-out) | Live | 1366x768 | pass | - | 1 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Add-ons | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/5630-controllers-addon-panel-tabs-1366x768-03-finding-1.png |
| F-002 | S4 | Add-ons | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/5630-controllers-addon-panel-tabs-1366x768-05-finding-2.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-001](shots/5630-controllers-addon-panel-tabs-1366x768-03-finding-1.png)

### F-002 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-002](shots/5630-controllers-addon-panel-tabs-1366x768-05-finding-2.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
