# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 7 run(s): 7 passed, 0 failed; 1 finding(s) (0 blocking, S1 to S3).

Program: `/home/claude/target/debug/kopercad`, built 1793900656 (unix time), source at git e5e66a9219.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Crane model: pick a STEP crane of your own, see it in place of the generic crane, change a role, undo (9610-crane-model-picker) | Simulate | 1366x768 | pass | - | 9 |
| Crane model: pick a STEP crane of your own, see it in place of the generic crane, change a role, undo (9610-crane-model-picker) | Simulate | 1920x1080 | pass | - | 9 |
| Crane model in motion: the hook block and hook of your own crane rise with a hoist run, the ropes follow (9611-crane-model-motion-hook) | Simulate | 1366x768 | pass | F-001 | 6 |
| Crane model in motion at 1920: the hook block and hook of your own crane rise with a hoist run, the ropes follow (9611b-crane-model-motion-hook-1920) | Simulate | 1920x1080 | pass | - | 5 |
| Crane model in motion: the bridge of your own crane travels along its rails with a travel drive (9612-crane-model-motion-travel) | Simulate | 1366x768 | pass | - | 3 |
| Crane model in motion at 1920: the bridge of your own crane travels along its rails with a travel drive (9612b-crane-model-motion-travel-1920) | Simulate | 1920x1080 | pass | - | 3 |
| Crane model is kept in the project: file, an edited role and the bridge axis survive save and reopen (9613-crane-model-saved) | Simulate | 1366x768 | pass | - | 3 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Simulate | 1366x768 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/9611-crane-model-motion-hook-1366x768-03-finding-1.png |

### F-001 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `9611-crane-model-motion-hook` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/hoist-two-speed.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 3`
5. `text Simulate hoist run`
6. `wait 4`
7. `key Enter`
8. `wait 20`
9. `click_text "Switch on Live"`
10. `wait 20`
11. `click_text "Start"`
12. `wait 40`
13. `key ctrl+k`
14. `wait 3`
15. `text Simulate hoist run`
16. `wait 4`
17. `key Enter`
18. `wait 20`
19. `key ctrl+k`
20. `wait 3`
21. `text Hoist in 3D`
22. `wait 4`
23. `key Enter`
24. `wait 30`
25. `answer_dialog qa/fixtures/crane-model/workshop-crane.stp`
26. `click_text "Crane model…"`
27. `wait 120`
28. `expect "workshop-crane.stp"`
29. `expect "Bridge"`
30. `expect "Trolley"`
31. `expect "Load"`
32. `shot own-crane-at-rest`
33. `check_layout`
34. `click_text "Parts"`
35. `wait 30`
36. `expect "Crane model parts"`
37. `expect "hook"`
38. `expect "trolley"`
39. `shot parts-moves-with`
40. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '✕ Close' over 'Hide'; 'Controller…' over 'wheel'

![F-001](shots/9611-crane-model-motion-hook-1366x768-03-finding-1.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
