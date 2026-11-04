# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 9 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/ct/debug/kopercad`, built 1793776085 (unix time), source at git cdcfa09b3a.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Lesson walk: wire-test (Test tab, reading, record, miss, list, PDF) (210-learn-wire-test) | Learn | 1366x768 | pass | - | 13 |
| Lesson walk: wire-test (Test tab, reading, record, miss, list, PDF) (210-learn-wire-test) | Learn | 1920x1080 | pass | F-001, F-002, F-003, F-004, F-005, F-006, F-007, F-008, F-009 | 22 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-05-finding-1.png |
| F-002 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-07-finding-2.png |
| F-003 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-09-finding-3.png |
| F-004 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-11-finding-4.png |
| F-005 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-13-finding-5.png |
| F-006 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-15-finding-6.png |
| F-007 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-18-finding-7.png |
| F-008 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-20-finding-8.png |
| F-009 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-22-finding-9.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-001](shots/210-learn-wire-test-1920x1080-05-finding-1.png)

### F-002 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-002](shots/210-learn-wire-test-1920x1080-07-finding-2.png)

### F-003 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-003](shots/210-learn-wire-test-1920x1080-09-finding-3.png)

### F-004 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`
34. `try click_text "Next"`
35. `wait 20`
36. `shot 07-record-step`
37. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-004](shots/210-learn-wire-test-1920x1080-11-finding-4.png)

### F-005 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`
34. `try click_text "Next"`
35. `wait 20`
36. `shot 07-record-step`
37. `check_layout`
38. `click_text Record Button`
39. `wait 20`
40. `expect "Recorded"`
41. `shot 08-recorded-miss-step`
42. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-005](shots/210-learn-wire-test-1920x1080-13-finding-5.png)

### F-006 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`
34. `try click_text "Next"`
35. `wait 20`
36. `shot 07-record-step`
37. `check_layout`
38. `click_text Record Button`
39. `wait 20`
40. `expect "Recorded"`
41. `shot 08-recorded-miss-step`
42. `check_layout`
43. `click_text "Open circuit" Button`
44. `wait 20`
45. `expect "MISS"`
46. `shot 09-miss`
47. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-006](shots/210-learn-wire-test-1920x1080-15-finding-6.png)

### F-007 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`
34. `try click_text "Next"`
35. `wait 20`
36. `shot 07-record-step`
37. `check_layout`
38. `click_text Record Button`
39. `wait 20`
40. `expect "Recorded"`
41. `shot 08-recorded-miss-step`
42. `check_layout`
43. `click_text "Open circuit" Button`
44. `wait 20`
45. `expect "MISS"`
46. `shot 09-miss`
47. `check_layout`
48. `click_text Punch Button`
49. `wait 20`
50. `shot 10-punch-tab`
51. `check_layout`
52. `click_text Test Button`
53. `wait 20`
54. `try click_text "Next"`
55. `wait 20`
56. `shot 11-list-step`
57. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-007](shots/210-learn-wire-test-1920x1080-18-finding-7.png)

### F-008 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`
34. `try click_text "Next"`
35. `wait 20`
36. `shot 07-record-step`
37. `check_layout`
38. `click_text Record Button`
39. `wait 20`
40. `expect "Recorded"`
41. `shot 08-recorded-miss-step`
42. `check_layout`
43. `click_text "Open circuit" Button`
44. `wait 20`
45. `expect "MISS"`
46. `shot 09-miss`
47. `check_layout`
48. `click_text Punch Button`
49. `wait 20`
50. `shot 10-punch-tab`
51. `check_layout`
52. `click_text Test Button`
53. `wait 20`
54. `try click_text "Next"`
55. `wait 20`
56. `shot 11-list-step`
57. `check_layout`
58. `try click_text "Next"`
59. `wait 20`
60. `shot 12-pdf-step`
61. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-008](shots/210-learn-wire-test-1920x1080-20-finding-8.png)

### F-009 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `210-learn-wire-test` at 1920x1080. Kind: layout-maybe.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text test every wire`
4. `wait 6`
5. `click_text "Test every wire: continuity and insulation"`
6. `wait 40`
7. `shot 01-why`
8. `check_layout`
9. `try click_text "Next"`
10. `wait 20`
11. `shot 02-open-commissioning`
12. `check_layout`
13. `try click_text "Do it for me"`
14. `wait 30`
15. `shot 03-tab-step`
16. `check_layout`
17. `click_text Test Button`
18. `wait 20`
19. `shot 04-sheet-step`
20. `check_layout`
21. `try click_text "Next"`
22. `wait 20`
23. `shot 05-reading-step`
24. `check_layout`
25. `click_text "" TextInput #1`
26. `wait 5`
27. `text 0.12`
28. `click_text "" TextInput #1`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`
34. `try click_text "Next"`
35. `wait 20`
36. `shot 07-record-step`
37. `check_layout`
38. `click_text Record Button`
39. `wait 20`
40. `expect "Recorded"`
41. `shot 08-recorded-miss-step`
42. `check_layout`
43. `click_text "Open circuit" Button`
44. `wait 20`
45. `expect "MISS"`
46. `shot 09-miss`
47. `check_layout`
48. `click_text Punch Button`
49. `wait 20`
50. `shot 10-punch-tab`
51. `check_layout`
52. `click_text Test Button`
53. `wait 20`
54. `try click_text "Next"`
55. `wait 20`
56. `shot 11-list-step`
57. `check_layout`
58. `try click_text "Next"`
59. `wait 20`
60. `shot 12-pdf-step`
61. `check_layout`
62. `click_text "Write test sheet PDF"`
63. `wait 30`
64. `shot 13-pdf-written`
65. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-009](shots/210-learn-wire-test-1920x1080-22-finding-9.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
