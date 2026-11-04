# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/tgt/debug/kopercad`, built 1793798941 (unix time), source at git 2f3a1113f5.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Parts table import: Dutch maker headers, price date, repeated order numbers, long list, palette ranking, folder picked as file (61-parts-table-audit) | Parts | 1366x768 | pass | F-001 | 11 |
| Parts table import: Dutch maker headers, price date, repeated order numbers, long list, palette ranking, folder picked as file (61-parts-table-audit) | Parts | 1920x1080 | pass | - | 10 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Parts | 1366x768 | 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/61-parts-table-audit-1366x768-11-finding-1.png |

### F-001 S4 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `61-parts-table-audit` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text import parts from a table`
6. `wait 10`
7. `shot palette`
8. `expect "Import parts from a table (CSV or Excel)"`
9. `expect_not "from a folder: from a table"`
10. `key Escape`
11. `wait 6`
12. `import_table qa/fixtures/parts/price-list-nl.csv`
13. `expect "Import parts from price-list-nl.csv"`
14. `expect "Prijsdatum"`
15. `expect "Price list date"`
16. `shot columns`
17. `check_layout`
18. `check_blank columns`
19. `click_text Rows Button`
20. `wait 10`
21. `click_text "Warning: Repeated order numbers" Button #1`
22. `wait 6`
23. `expect "more than one row"`
24. `click_text "Warning: Repeated order numbers" Button #1`
25. `wait 6`
26. `click_text "Warning: Check this row" Button #1`
27. `wait 6`
28. `expect "is not a date"`
29. `expect "not for pieces"`
30. `click_text "Warning: Check this row" Button #1`
31. `wait 6`
32. `shot rows-last`
33. `check_layout`
34. `click_text "First row" Button`
35. `wait 10`
36. `shot rows-first`
37. `check_layout`
38. `click_text "Add 4 parts to the catalog"`
39. `wait 20`
40. `shot added`
41. `answer_dialog qa/fixtures/parts/price-list-nl.csv`
42. `click_text File`
43. `wait 4`
44. `click_text "Import..."`
45. `wait 4`
46. `text "folder"`
47. `wait 4`
48. `click_text "Import parts folder"`
49. `wait 2`
50. `click_text "Import" Button #1`
51. `wait 20`
52. `expect "is a file, not a folder"`
53. `expect_not "os error"`
54. `shot folder-picked-file`
55. `import_table qa/fixtures/parts/odd-list.xlsx`
56. `expect "Import parts from odd-list.xlsx"`
57. `shot xlsx-columns`
58. `click_text Cancel`
59. `wait 6`
60. `import_table qa/fixtures/parts/not-text.csv`
61. `expect "does not look like a text file"`
62. `shot refused`
63. `import_table qa/fixtures/parts/price-list-long.csv`
64. `click_text Rows Button`
65. `wait 10`
66. `expect "Repeated"`
67. `shot rows-long`
68. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cable' over 'Columns'; 'Add text' over 'Warning: Repeated order numbers'; 'Warning: Repeated' over 'Add 40 parts to the catalog'

![F-001](shots/61-parts-table-audit-1366x768-11-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
