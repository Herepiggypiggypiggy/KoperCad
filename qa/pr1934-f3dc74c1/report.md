# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 8 run(s): 8 passed, 0 failed; 12 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/tgt/debug/kopercad`, built 1793835960 (unix time), source at git f3dc74c156.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Calculations > Mechanical shows a busbar, a rail and a plate result (186700-calc-mechanical) | Calculations | 1366x768 | pass | F-001, F-002 | 9 |
| Calculations > Mechanical shows a busbar, a rail and a plate result (186700-calc-mechanical) | Calculations | 1920x1080 | pass | F-003, F-004 | 9 |
| Calculations > Mechanical with no supply current stated (186701-calc-mechanical-no-supply) | Calculations | 1366x768 | pass | - | 1 |
| Calculations > Mechanical with no supply current stated (186701-calc-mechanical-no-supply) | Calculations | 1920x1080 | pass | - | 1 |
| Calculations > Mechanical: a rail with no load is not calculated (186702-calc-mechanical-bad-input-1366) | Calculations | 1366x768 | pass | F-005 | 3 |
| Calculations > Mechanical: a rail with no load is not calculated (186703-calc-mechanical-bad-input-1920) | Calculations | 1920x1080 | pass | F-006 | 3 |
| Calculations > Busbar impedance: bar impedance and current sharing with the model label (18740-calc-busbars) | Windows | 1366x768 | pass | F-007, F-008, F-009, F-010, F-011, F-012 | 12 |
| Calculations > Busbar impedance: bar impedance and current sharing with the model label (18740-calc-busbars) | Windows | 1920x1080 | pass | - | 6 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Calculations | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/186700-calc-mechanical-1366x768-05-finding-1.png |
| F-002 | S4 | Calculations | 1366x768 | 5 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/186700-calc-mechanical-1366x768-09-finding-2.png |
| F-003 | S4 | Calculations | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/186700-calc-mechanical-1920x1080-05-finding-3.png |
| F-004 | S4 | Calculations | 1920x1080 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/186700-calc-mechanical-1920x1080-09-finding-4.png |
| F-005 | S4 | Calculations | 1366x768 | 8 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/186702-calc-mechanical-bad-input-1366-1366x768-03-finding-5.png |
| F-006 | S4 | Calculations | 1920x1080 | 6 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/186703-calc-mechanical-bad-input-1920-1920x1080-03-finding-6.png |
| F-007 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-02-finding-7.png |
| F-008 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-04-finding-8.png |
| F-009 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-06-finding-9.png |
| F-010 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-08-finding-10.png |
| F-011 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-10-finding-11.png |
| F-012 | S4 | Windows | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18740-calc-busbars-1366x768-12-finding-12.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `186700-calc-mechanical` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text calculations`
6. `wait 8`
7. `key Enter`
8. `wait 15`
9. `shot circuits-tab`
10. `click_text "Panel SCCR"`
11. `wait 10`
12. `shot panel-sccr-tab`
13. `click_text "Motor starters"`
14. `wait 10`
15. `shot starters-tab`
16. `click_text "Mechanical"`
17. `wait 10`
18. `shot mechanical-top`
19. `expect "Estimate, not a verification of the busbar or support rating"`
20. `expect "Not a safety function"`
21. `expect "Bending stress"`
22. `expect "Force on one support"`
23. `expect "swings at"`
24. `expect "at the supply"`
25. `expect "Check > Calculations > Panel SCCR"`
26. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '3D' over 'Close window'

![F-001](shots/186700-calc-mechanical-1366x768-05-finding-1.png)

### F-002 S4 5 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `186700-calc-mechanical` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text calculations`
6. `wait 8`
7. `key Enter`
8. `wait 15`
9. `shot circuits-tab`
10. `click_text "Panel SCCR"`
11. `wait 10`
12. `shot panel-sccr-tab`
13. `click_text "Motor starters"`
14. `wait 10`
15. `shot starters-tab`
16. `click_text "Mechanical"`
17. `wait 10`
18. `shot mechanical-top`
19. `expect "Estimate, not a verification of the busbar or support rating"`
20. `expect "Not a safety function"`
21. `expect "Bending stress"`
22. `expect "Force on one support"`
23. `expect "swings at"`
24. `expect "at the supply"`
25. `expect "Check > Calculations > Panel SCCR"`
26. `check_layout`
27. `scroll 400 450 -300`
28. `wait 6`
29. `shot mechanical-rail`
30. `scroll 400 450 -400`
31. `wait 6`
32. `shot mechanical-rail-and-plate`
33. `expect "Deflection"`
34. `click_text "Mounting plate under device weight"`
35. `wait 6`
36. `scroll 400 450 -400`
37. `wait 6`
38. `shot mechanical-plate`
39. `expect "at the centre"`
40. `tree Mechanical`
41. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Draw' over 'Help'; '3D' over 'Close window'; 'Motor starters' over 'Help'; 'Busbar impedance' over 'In the plate plane'; 'Mechanical' over 'Out of the plate'

![F-002](shots/186700-calc-mechanical-1366x768-09-finding-2.png)

### F-003 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `186700-calc-mechanical` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text calculations`
6. `wait 8`
7. `key Enter`
8. `wait 15`
9. `shot circuits-tab`
10. `click_text "Panel SCCR"`
11. `wait 10`
12. `shot panel-sccr-tab`
13. `click_text "Motor starters"`
14. `wait 10`
15. `shot starters-tab`
16. `click_text "Mechanical"`
17. `wait 10`
18. `shot mechanical-top`
19. `expect "Estimate, not a verification of the busbar or support rating"`
20. `expect "Not a safety function"`
21. `expect "Bending stress"`
22. `expect "Force on one support"`
23. `expect "swings at"`
24. `expect "at the supply"`
25. `expect "Check > Calculations > Panel SCCR"`
26. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Drawn wires (27)' over 'Help'

![F-003](shots/186700-calc-mechanical-1920x1080-05-finding-3.png)

### F-004 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `186700-calc-mechanical` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text calculations`
6. `wait 8`
7. `key Enter`
8. `wait 15`
9. `shot circuits-tab`
10. `click_text "Panel SCCR"`
11. `wait 10`
12. `shot panel-sccr-tab`
13. `click_text "Motor starters"`
14. `wait 10`
15. `shot starters-tab`
16. `click_text "Mechanical"`
17. `wait 10`
18. `shot mechanical-top`
19. `expect "Estimate, not a verification of the busbar or support rating"`
20. `expect "Not a safety function"`
21. `expect "Bending stress"`
22. `expect "Force on one support"`
23. `expect "swings at"`
24. `expect "at the supply"`
25. `expect "Check > Calculations > Panel SCCR"`
26. `check_layout`
27. `scroll 400 450 -300`
28. `wait 6`
29. `shot mechanical-rail`
30. `scroll 400 450 -400`
31. `wait 6`
32. `shot mechanical-rail-and-plate`
33. `expect "Deflection"`
34. `click_text "Mounting plate under device weight"`
35. `wait 6`
36. `scroll 400 450 -400`
37. `wait 6`
38. `shot mechanical-plate`
39. `expect "at the centre"`
40. `tree Mechanical`
41. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinets' over 'Help'; 'Motor starters' over 'Help'

![F-004](shots/186700-calc-mechanical-1920x1080-09-finding-4.png)

### F-005 S4 8 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `186702-calc-mechanical-bad-input-1366` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text calculations`
6. `wait 8`
7. `key Enter`
8. `wait 15`
9. `click_text "Mechanical"`
10. `wait 10`
11. `scroll 400 450 -300`
12. `wait 6`
13. `scroll 400 450 -400`
14. `wait 6`
15. `shot rail-before`
16. `double_click 329 510`
17. `wait 4`
18. `key ctrl+a`
19. `text 0`
20. `key Enter`
21. `wait 6`
22. `shot rail-no-load`
23. `expect "Not calculated"`
24. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Edit' over 'Help'; 'Reports' over 'one span, free ends'; 'Simulate' over 'one span, free ends'; 'Help' over 'one span, clamped ends'; 'Schematic' over 'two equal spans'

![F-005](shots/186702-calc-mechanical-bad-input-1366-1366x768-03-finding-5.png)

### F-006 S4 6 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `186703-calc-mechanical-bad-input-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text calculations`
6. `wait 8`
7. `key Enter`
8. `wait 15`
9. `click_text "Mechanical"`
10. `wait 10`
11. `scroll 400 450 -300`
12. `wait 6`
13. `scroll 400 450 -400`
14. `wait 6`
15. `shot rail-before`
16. `double_click 329 751`
17. `wait 4`
18. `key ctrl+a`
19. `text 0`
20. `key Enter`
21. `wait 6`
22. `shot rail-no-load`
23. `expect "Not calculated"`
24. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cables (1)' over 'DIN rail under device weight'; 'Hide' over 'Busbar short circuit'; 'Motor starters' over 'Help'; 'Busbar impedance' over 'Three-phase'; 'Busbar impedance' over 'Two conductors'

![F-006](shots/186703-calc-mechanical-bad-input-1920-1920x1080-03-finding-6.png)

### F-007 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-007](shots/18740-calc-busbars-1366x768-02-finding-7.png)

### F-008 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-008](shots/18740-calc-busbars-1366x768-04-finding-8.png)

### F-009 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-009](shots/18740-calc-busbars-1366x768-06-finding-9.png)

### F-010 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-010](shots/18740-calc-busbars-1366x768-08-finding-10.png)

### F-011 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-011](shots/18740-calc-busbars-1366x768-10-finding-11.png)

### F-012 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-012](shots/18740-calc-busbars-1366x768-12-finding-12.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
