# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 3 run(s): 1 passed, 2 failed; 7 finding(s) (7 blocking, S1 to S3).

Program: `/tmp/ktarget/debug/kopercad`, built 1793624544 (unix time), source at git 49396165ab.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Cabinet: a device dragged near its rail clips onto it by its centre, one dragged away leaves it (79-cabinet-rail-snap) | Cabinet | 1366x768 | fail | F-001, F-002, F-003, F-004 | 9 |
| Cabinet: field, door and plate (Mounted row, Size enclosure note, Issues, Place all) (1700-cabinet-sites) | Cabinet | 1366x768 | fail | F-005, F-006, F-007 | 5 |
| Cabinet: field, door and plate (Mounted row, Size enclosure note, Issues, Place all) (1700-cabinet-sites) | Cabinet | 1920x1080 | pass | - | 9 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S3 | Cabinet | 1366x768 | `55, 210 mm` not on screen | shots/79-cabinet-rail-snap-1366x768-03-finding-1.png |
| F-002 | S3 | Cabinet | 1366x768 | `Mounted on` not on screen | shots/79-cabinet-rail-snap-1366x768-04-finding-2.png |
| F-003 | S3 | Cabinet | 1366x768 | `210 mm` not on screen | shots/79-cabinet-rail-snap-1366x768-06-finding-3.png |
| F-004 | S3 | Cabinet | 1366x768 | `Mounted on` not on screen | shots/79-cabinet-rail-snap-1366x768-07-finding-4.png |
| F-005 | S3 | Cabinet | 1366x768 | `Mounted` not on screen | shots/1700-cabinet-sites-1366x768-03-finding-5.png |
| F-006 | S3 | Cabinet | 1366x768 | `In the field` not on screen | shots/1700-cabinet-sites-1366x768-04-finding-6.png |
| F-007 | S3 | Cabinet | 1366x768 | step failed: click_text "In the field" | shots/1700-cabinet-sites-1366x768-05-finding-7.png |

### F-001 S3 `55, 210 mm` not on screen

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

![F-001](shots/79-cabinet-rail-snap-1366x768-03-finding-1.png)

### F-002 S3 `Mounted on` not on screen

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

![F-002](shots/79-cabinet-rail-snap-1366x768-04-finding-2.png)

### F-003 S3 `210 mm` not on screen

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

![F-003](shots/79-cabinet-rail-snap-1366x768-06-finding-3.png)

### F-004 S3 `Mounted on` not on screen

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

![F-004](shots/79-cabinet-rail-snap-1366x768-07-finding-4.png)

### F-005 S3 `Mounted` not on screen

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

![F-005](shots/1700-cabinet-sites-1366x768-03-finding-5.png)

### F-006 S3 `In the field` not on screen

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

![F-006](shots/1700-cabinet-sites-1366x768-04-finding-6.png)

### F-007 S3 step failed: click_text "In the field"

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

![F-007](shots/1700-cabinet-sites-1366x768-05-finding-7.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
