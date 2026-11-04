# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 6 run(s): 6 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/tgt/debug/kopercad`, built 1793798941 (unix time), source at git 2f3a1113f5.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Problem triangles T9: a parts table row that cannot be imported has a triangle with the cell detail (9490-parts-table-row-triangle) | Parts | 1366x768 | pass | - | 3 |
| Problem triangles T9: a parts table row that cannot be imported has a triangle with the cell detail (9490-parts-table-row-triangle) | Parts | 1920x1080 | pass | - | 3 |
| Problem triangles T9: a table that cannot be read leaves a triangle in the Parts window, Dismiss clears it (9491-parts-job-triangle) | Parts | 1366x768 | pass | - | 3 |
| Problem triangles T9: a table that cannot be read leaves a triangle in the Parts window, Dismiss clears it (9491-parts-job-triangle) | Parts | 1920x1080 | pass | F-001 | 4 |
| Problem triangles T9: columns that cannot be read show one triangle instead of red sentences (9492-parts-columns-triangle) | Parts | 1366x768 | pass | - | 1 |
| Problem triangles T9: columns that cannot be read show one triangle instead of red sentences (9492-parts-columns-triangle) | Parts | 1920x1080 | pass | - | 1 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Parts | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/9491-parts-job-triangle-1920x1080-02-finding-1.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `9491-parts-job-triangle` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `text Parts`
5. `wait 10`
6. `key Return`
7. `wait 20`
8. `answer_dialog qa/fixtures/parts/not-text.csv`
9. `click_text "Import table…"`
10. `wait 20`
11. `shot triangle-in-window`
12. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Parts'

![F-001](shots/9491-parts-job-triangle-1920x1080-02-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
