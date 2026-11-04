# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 2 run(s): 2 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target/debug/kopercad`, built 1793778711 (unix time), source at git ef4122ce09.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Parts window: Get parts from supplier menu and the fallback without a web view (18640-parts-get-from-supplier) | Parts | 1366x768 | pass | - | 2 |
| Parts window: Get parts from supplier menu and the fallback without a web view (18640-parts-get-from-supplier) | Parts | 1920x1080 | pass | F-001 | 3 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Parts | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/18640-parts-get-from-supplier-1920x1080-03-finding-1.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `18640-parts-get-from-supplier` at 1920x1080. Kind: layout-maybe.

Steps:

1. `fresh`
2. `click_text "New project"`
3. `wait 20`
4. `key ctrl+k`
5. `text Parts`
6. `wait 10`
7. `key Return`
8. `wait 20`
9. `expect "Get parts from supplier"`
10. `click_text "Get parts from supplier"`
11. `wait 10`
12. `expect "Rittal PARTcommunity"`
13. `expect "Use my normal browser"`
14. `shot supplier-menu`
15. `click_text "Rittal PARTcommunity"`
16. `wait 15`
17. `expect "WebView2"`
18. `shot fallback-no-webview`
19. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Parts'

![F-001](shots/18640-parts-get-from-supplier-1920x1080-03-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
