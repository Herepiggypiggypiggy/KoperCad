# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 6 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793804315 (unix time), source at git 8ed8992b75.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Calculations > Busbar impedance: bar impedance and current sharing with the model label (18740-calc-busbars) | Windows | 1366x768 | pass | F-001, F-002, F-003, F-004, F-005, F-006 | 12 |
| Calculations > Busbar impedance: bar impedance and current sharing with the model label (18740-calc-busbars) | Windows | 1920x1080 | pass | - | 6 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-02-finding-1.png |
| F-002 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-04-finding-2.png |
| F-003 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-06-finding-3.png |
| F-004 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-08-finding-4.png |
| F-005 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-10-finding-5.png |
| F-006 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-12-finding-6.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18740-calc-busbars` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text calculations`
5. `key enter`
6. `wait 10`
7. `click_text "Busbar impedance"`
8. `wait 20`
9. `expect model, quasi-static`
10. `shot busbars-one-bar`
11. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-001](shots/18740-calc-busbars-1366x768-02-finding-1.png)

### F-002 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18740-calc-busbars` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text calculations`
5. `key enter`
6. `wait 10`
7. `click_text "Busbar impedance"`
8. `wait 20`
9. `expect model, quasi-static`
10. `shot busbars-one-bar`
11. `check_layout`
12. `click_text "2 bars"`
13. `wait 10`
14. `scroll 400 450 -600`
15. `wait 5`
16. `expect hottest`
17. `expect Share`
18. `shot busbars-two-bars-stacked`
19. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-002](shots/18740-calc-busbars-1366x768-04-finding-2.png)

### F-003 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18740-calc-busbars` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text calculations`
5. `key enter`
6. `wait 10`
7. `click_text "Busbar impedance"`
8. `wait 20`
9. `expect model, quasi-static`
10. `shot busbars-one-bar`
11. `check_layout`
12. `click_text "2 bars"`
13. `wait 10`
14. `scroll 400 450 -600`
15. `wait 5`
16. `expect hottest`
17. `expect Share`
18. `shot busbars-two-bars-stacked`
19. `check_layout`
20. `scroll 400 450 600`
21. `wait 5`
22. `click_text "side by side"`
23. `wait 10`
24. `shot busbars-unequal-sharing`
25. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-003](shots/18740-calc-busbars-1366x768-06-finding-3.png)

### F-004 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18740-calc-busbars` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text calculations`
5. `key enter`
6. `wait 10`
7. `click_text "Busbar impedance"`
8. `wait 20`
9. `expect model, quasi-static`
10. `shot busbars-one-bar`
11. `check_layout`
12. `click_text "2 bars"`
13. `wait 10`
14. `scroll 400 450 -600`
15. `wait 5`
16. `expect hottest`
17. `expect Share`
18. `shot busbars-two-bars-stacked`
19. `check_layout`
20. `scroll 400 450 600`
21. `wait 5`
22. `click_text "side by side"`
23. `wait 10`
24. `shot busbars-unequal-sharing`
25. `check_layout`
26. `scroll 400 450 -600`
27. `wait 5`
28. `shot busbars-per-bar-lower`
29. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-004](shots/18740-calc-busbars-1366x768-08-finding-4.png)

### F-005 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18740-calc-busbars` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text calculations`
5. `key enter`
6. `wait 10`
7. `click_text "Busbar impedance"`
8. `wait 20`
9. `expect model, quasi-static`
10. `shot busbars-one-bar`
11. `check_layout`
12. `click_text "2 bars"`
13. `wait 10`
14. `scroll 400 450 -600`
15. `wait 5`
16. `expect hottest`
17. `expect Share`
18. `shot busbars-two-bars-stacked`
19. `check_layout`
20. `scroll 400 450 600`
21. `wait 5`
22. `click_text "side by side"`
23. `wait 10`
24. `shot busbars-unequal-sharing`
25. `check_layout`
26. `scroll 400 450 -600`
27. `wait 5`
28. `shot busbars-per-bar-lower`
29. `check_layout`
30. `click_text "Frequency sweep"`
31. `wait 40`
32. `shot busbars-sweep`
33. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-005](shots/18740-calc-busbars-1366x768-10-finding-5.png)

### F-006 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18740-calc-busbars` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text calculations`
5. `key enter`
6. `wait 10`
7. `click_text "Busbar impedance"`
8. `wait 20`
9. `expect model, quasi-static`
10. `shot busbars-one-bar`
11. `check_layout`
12. `click_text "2 bars"`
13. `wait 10`
14. `scroll 400 450 -600`
15. `wait 5`
16. `expect hottest`
17. `expect Share`
18. `shot busbars-two-bars-stacked`
19. `check_layout`
20. `scroll 400 450 600`
21. `wait 5`
22. `click_text "side by side"`
23. `wait 10`
24. `shot busbars-unequal-sharing`
25. `check_layout`
26. `scroll 400 450 -600`
27. `wait 5`
28. `shot busbars-per-bar-lower`
29. `check_layout`
30. `click_text "Frequency sweep"`
31. `wait 40`
32. `shot busbars-sweep`
33. `check_layout`
34. `click_text "Copy as text"`
35. `wait 5`
36. `click_text Circuits`
37. `wait 10`
38. `shot circuits-still-there`
39. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-006](shots/18740-calc-busbars-1366x768-12-finding-6.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
