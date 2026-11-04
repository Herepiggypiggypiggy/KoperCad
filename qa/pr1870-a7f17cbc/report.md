# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 6 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793779206 (unix time), source at git a7f17cbc8a.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Parts: Add CAD files shows the STEP model health line (1870-step-health-line) | Parts | 1366x768 | pass | F-001, F-002, F-003 | 7 |
| Parts: Add CAD files shows the STEP model health line (1870-step-health-line) | Parts | 1920x1080 | pass | F-004, F-005, F-006 | 7 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Parts | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1870-step-health-line-1366x768-02-finding-1.png |
| F-002 | S4 | Parts | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1870-step-health-line-1366x768-05-finding-2.png |
| F-003 | S4 | Parts | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1870-step-health-line-1366x768-07-finding-3.png |
| F-004 | S4 | Parts | 1920x1080 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1870-step-health-line-1920x1080-02-finding-4.png |
| F-005 | S4 | Parts | 1920x1080 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1870-step-health-line-1920x1080-05-finding-5.png |
| F-006 | S4 | Parts | 1920x1080 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1870-step-health-line-1920x1080-07-finding-6.png |

### F-001 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1870-step-health-line` at 1366x768. Kind: layout-maybe.

Steps:

1. `fresh`
2. `key ctrl+k`
3. `text "parts"`
4. `wait 10`
5. `key Return`
6. `wait 20`
7. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_clean_plate.stp`
8. `click_text "Add CAD files"`
9. `wait 40`
10. `expect "Add CAD files"`
11. `expect "Reads cleanly"`
12. `expect "Mounting plate"`
13. `shot clean-plate`
14. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'New project' over 'Tick all probable'; 'New project' over 'Close'

![F-001](shots/1870-step-health-line-1366x768-02-finding-1.png)

### F-002 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1870-step-health-line` at 1366x768. Kind: layout-maybe.

Steps:

1. `fresh`
2. `key ctrl+k`
3. `text "parts"`
4. `wait 10`
5. `key Return`
6. `wait 20`
7. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_clean_plate.stp`
8. `click_text "Add CAD files"`
9. `wait 40`
10. `expect "Add CAD files"`
11. `expect "Reads cleanly"`
12. `expect "Mounting plate"`
13. `shot clean-plate`
14. `check_layout`
15. `click 276 229`
16. `wait 15`
17. `expect "kopercad-cli parts step-health"`
18. `shot details`
19. `click 276 229`
20. `wait 10`
21. `click_text "Close"`
22. `wait 10`
23. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_gaps.stp`
24. `click_text "Add CAD files"`
25. `wait 40`
26. `expect "Reads with gaps"`
27. `shot gaps`
28. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'New project' over 'Tick all probable'; 'New project' over 'Close'

![F-002](shots/1870-step-health-line-1366x768-05-finding-2.png)

### F-003 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1870-step-health-line` at 1366x768. Kind: layout-maybe.

Steps:

1. `fresh`
2. `key ctrl+k`
3. `text "parts"`
4. `wait 10`
5. `key Return`
6. `wait 20`
7. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_clean_plate.stp`
8. `click_text "Add CAD files"`
9. `wait 40`
10. `expect "Add CAD files"`
11. `expect "Reads cleanly"`
12. `expect "Mounting plate"`
13. `shot clean-plate`
14. `check_layout`
15. `click 276 229`
16. `wait 15`
17. `expect "kopercad-cli parts step-health"`
18. `shot details`
19. `click 276 229`
20. `wait 10`
21. `click_text "Close"`
22. `wait 10`
23. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_gaps.stp`
24. `click_text "Add CAD files"`
25. `wait 40`
26. `expect "Reads with gaps"`
27. `shot gaps`
28. `check_layout`
29. `click_text "Close"`
30. `wait 10`
31. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_broken.stp`
32. `click_text "Add CAD files"`
33. `wait 40`
34. `expect "Cannot be read"`
35. `shot broken`
36. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'New project' over 'Tick all probable'; 'New project' over 'Close'

![F-003](shots/1870-step-health-line-1366x768-07-finding-3.png)

### F-004 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1870-step-health-line` at 1920x1080. Kind: layout-maybe.

Steps:

1. `fresh`
2. `key ctrl+k`
3. `text "parts"`
4. `wait 10`
5. `key Return`
6. `wait 20`
7. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_clean_plate.stp`
8. `click_text "Add CAD files"`
9. `wait 40`
10. `expect "Add CAD files"`
11. `expect "Reads cleanly"`
12. `expect "Mounting plate"`
13. `shot clean-plate`
14. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'New project' over 'Tick all probable'; 'New project' over 'Close'

![F-004](shots/1870-step-health-line-1920x1080-02-finding-4.png)

### F-005 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1870-step-health-line` at 1920x1080. Kind: layout-maybe.

Steps:

1. `fresh`
2. `key ctrl+k`
3. `text "parts"`
4. `wait 10`
5. `key Return`
6. `wait 20`
7. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_clean_plate.stp`
8. `click_text "Add CAD files"`
9. `wait 40`
10. `expect "Add CAD files"`
11. `expect "Reads cleanly"`
12. `expect "Mounting plate"`
13. `shot clean-plate`
14. `check_layout`
15. `click 276 229`
16. `wait 15`
17. `expect "kopercad-cli parts step-health"`
18. `shot details`
19. `click 276 229`
20. `wait 10`
21. `click_text "Close"`
22. `wait 10`
23. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_gaps.stp`
24. `click_text "Add CAD files"`
25. `wait 40`
26. `expect "Reads with gaps"`
27. `shot gaps`
28. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'New project' over 'Tick all probable'; 'New project' over 'Close'

![F-005](shots/1870-step-health-line-1920x1080-05-finding-5.png)

### F-006 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1870-step-health-line` at 1920x1080. Kind: layout-maybe.

Steps:

1. `fresh`
2. `key ctrl+k`
3. `text "parts"`
4. `wait 10`
5. `key Return`
6. `wait 20`
7. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_clean_plate.stp`
8. `click_text "Add CAD files"`
9. `wait 40`
10. `expect "Add CAD files"`
11. `expect "Reads cleanly"`
12. `expect "Mounting plate"`
13. `shot clean-plate`
14. `check_layout`
15. `click 276 229`
16. `wait 15`
17. `expect "kopercad-cli parts step-health"`
18. `shot details`
19. `click 276 229`
20. `wait 10`
21. `click_text "Close"`
22. `wait 10`
23. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_gaps.stp`
24. `click_text "Add CAD files"`
25. `wait 40`
26. `expect "Reads with gaps"`
27. `shot gaps`
28. `check_layout`
29. `click_text "Close"`
30. `wait 10`
31. `answer_dialog qa/fixtures/parts/cad/HEALTH1870_broken.stp`
32. `click_text "Add CAD files"`
33. `wait 40`
34. `expect "Cannot be read"`
35. `shot broken`
36. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'New project' over 'Tick all probable'; 'New project' over 'Close'

![F-006](shots/1870-step-health-line-1920x1080-07-finding-6.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
