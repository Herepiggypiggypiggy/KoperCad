# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 40 run(s): 34 passed, 6 failed; 12 finding(s) (7 blocking, S1 to S3).

Program: `/tmp/kc-target2/debug/kopercad`, built 1793776555 (unix time), source at git a18d57e9fe.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| 3D: heat and EMC switch off from the toolbar and stay off, one switch each (1599-3d-panels-audit) | 3D | 1366x768 | pass | - | 8 |
| 3D: heat and EMC switch off from the toolbar and stay off, one switch each (1599-3d-panels-audit) | 3D | 1920x1080 | pass | - | 8 |
| Heat legend under a lesson card: the scale lies flat, ticks are readable, the notes scroll (181-legend-with-tall-card) | Cabinet | 1366x768 | pass | F-001 | 7 |
| Heat legend under a lesson card: the scale lies flat, ticks are readable, the notes scroll (181-legend-with-tall-card) | Cabinet | 1920x1080 | pass | F-002 | 7 |
| 3D Heat vents: before any run the openings show no measured flow and the panel says why (1815-3d-vents-no-run-wording) | 3D | 1366x768 | pass | - | 2 |
| 3D Heat vents: before any run the openings show no measured flow and the panel says why (1815-3d-vents-no-run-wording) | 3D | 1920x1080 | pass | - | 2 |
| Work cards: printed, then a cooler layout is applied, the strip says the cards are out of date (230-workpack-out-of-date-after-cooler-layout) | 3D | 1366x768 | fail | F-003, F-004 | 8 |
| 3D heat strip: airflow, shell, heat flow and devices on in a short window keep the panels in place (270-3d-heat-strip-two-layer-rows-short-window) | 3D | 1280x688 | fail | F-005 | 1 |
| 3D heat strip: airflow, shell, heat flow and devices on in a short window keep the panels in place (270-3d-heat-strip-two-layer-rows-short-window) | 3D | 1366x768 | pass | - | 2 |
| Heat: the airflow lines keep a light edge in the Light theme (300-heat-airflow-lines-light) | Cabinet | 1366x768 | pass | - | 3 |
| Heat: the airflow lines keep a light edge in the Light theme (300-heat-airflow-lines-light) | Cabinet | 1920x1080 | pass | - | 3 |
| Cooling advice: after adding a pair, the 3D run is put next to the 2D prediction (380-3d-cooling-advice-2d-vs-3d-comparison) | 3D | 1366x768 | fail | F-006 | 4 |
| Cooling advice: after adding a pair, the 3D run is put next to the 2D prediction (380-3d-cooling-advice-2d-vs-3d-comparison) | 3D | 1920x1080 | fail | F-007 | 4 |
| Cooling what-if: conditions, ranked fixes with a before and after table, 3D preview, apply (400-3d-cooling-whatif) | 3D | 1366x768 | pass | - | 6 |
| Cooling what-if: conditions, ranked fixes with a before and after table, 3D preview, apply (400-3d-cooling-whatif) | 3D | 1920x1080 | pass | - | 6 |
| Cooling what-if in the Light look (401-3d-cooling-whatif-light) | 3D | 1366x768 | pass | - | 3 |
| Cooling what-if: a layout move previewed in 3D (403-3d-cooling-whatif-layout-preview) | 3D | 1366x768 | pass | F-008 | 4 |
| Cooling what-if folds to its title bar so the 3D preview is visible (404-3d-cooling-whatif-collapsed) | 3D | 1366x768 | pass | - | 2 |
| Cooling what-if: a fix with fans carries the fan note (405-3d-cooling-whatif-fan-note) | 3D | 1366x768 | pass | - | 1 |
| Cooling what-if: a fix with fans carries the fan note (405-3d-cooling-whatif-fan-note) | 3D | 1920x1080 | pass | - | 1 |
| Cooling what-if: a layout move previewed in 3D (1920 layout) (406-3d-cooling-whatif-layout-preview-1920) | 3D | 1920x1080 | pass | F-009 | 4 |
| Cooling what-if: a fan fix names the devices beside its stream (407-3d-cooling-whatif-beside-stream) | 3D | 1366x768 | pass | - | 1 |
| Cooling what-if: a fan fix names the devices beside its stream (407-3d-cooling-whatif-beside-stream) | 3D | 1920x1080 | pass | - | 1 |
| Cooling what-if: the beside-the-stream line in the Light look (408-3d-cooling-whatif-beside-stream-light) | 3D | 1366x768 | pass | - | 1 |
| 3D view: Heat panel, Simulate, verdict and pinned hot-spot labels (42-3d-heat) | 3D | 1366x768 | pass | - | 4 |
| 3D view: Heat panel, Simulate, verdict and pinned hot-spot labels (42-3d-heat) | 3D | 1920x1080 | pass | - | 4 |
| 3D Heat: hold the colour range, then go back to Auto (50-3d-heat-scale) | 3D | 1366x768 | pass | - | 3 |
| 3D Heat: hold the colour range, then go back to Auto (50-3d-heat-scale) | 3D | 1920x1080 | pass | - | 3 |
| 3D Heat: vents and fans drawn in the cabinet, airflow coloured by speed (75-3d-heat-vents) | 3D | 1366x768 | pass | - | 12 |
| 3D Heat: vents and fans drawn in the cabinet, airflow coloured by speed (75-3d-heat-vents) | 3D | 1920x1080 | pass | - | 12 |
| 3D Heat vents: the row under the pointer marks its wall in the cabinet (77-3d-heat-vents-hover) | 3D | 1366x768 | pass | - | 3 |
| 3D view: EMC view of the drive example (classes, keep-clear bands, hot spots, card, legend, light look) (96-3d-emc) | 3D | 1366x768 | pass | - | 4 |
| 3D view: EMC view of the drive example (classes, keep-clear bands, hot spots, card, legend, light look) (96-3d-emc) | 3D | 1920x1080 | pass | - | 4 |
| 3D Heat: a fan thinner than any common cabinet fan shows a warning (98-thin-fan-warning) | 3D | 1366x768 | pass | - | 6 |
| 3D Heat: a fan thinner than any common cabinet fan shows a warning (98-thin-fan-warning) | 3D | 1920x1080 | pass | - | 6 |
| 3D Heat: the status line sits on its own row, and the Cooling advice window shows its notes once (112-3d-heat-panel-row-and-advice) | 3D | 1280x688 | pass | F-010 | 4 |
| 3D Heat: the status line sits on its own row, and the Cooling advice window shows its notes once (112-3d-heat-panel-row-and-advice) | 3D | 1366x768 | pass | - | 3 |
| 3D Heat: the status line sits on its own row, and the Cooling advice window shows its notes once (112-3d-heat-panel-row-and-advice) | 3D | 1920x1080 | pass | - | 3 |
| Heat run refuses an enclosure over 20 m on a side (16176-heat-refuses-20m-enclosure) | Heat | 1366x768 | fail | F-011 | 2 |
| Heat run refuses an enclosure over 20 m on a side (16176-heat-refuses-20m-enclosure) | Heat | 1920x1080 | fail | F-012 | 2 |

## Files kept

Exports the run wrote, copied next to this report:

- `files/230-workpack-out-of-date-after-cooler-layout-1366x768-qa_230_work_cards_again.pdf`
- `files/230-workpack-out-of-date-after-cooler-layout-1366x768-qa_230_work_cards.pdf`

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Cabinet | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/181-legend-with-tall-card-1366x768-05-finding-1.png |
| F-002 | S4 | Cabinet | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/181-legend-with-tall-card-1920x1080-05-finding-2.png |
| F-003 | S3 | 3D | 1366x768 | `Work cards out of date` not on screen | shots/230-workpack-out-of-date-after-cooler-layout-1366x768-05-finding-3.png |
| F-004 | S3 | 3D | 1366x768 | `Work cards out of date` not on screen | shots/230-workpack-out-of-date-after-cooler-layout-1366x768-07-finding-4.png |
| F-005 | S3 | 3D | 1280x688 | step failed: click_text Quick | shots/270-3d-heat-strip-two-layer-rows-short-window-1280x688-01-finding-5.png |
| F-006 | S3 | 3D | 1366x768 | `Model comparison` not on screen | shots/380-3d-cooling-advice-2d-vs-3d-comparison-1366x768-02-finding-6.png |
| F-007 | S3 | 3D | 1920x1080 | `Model comparison` not on screen | shots/380-3d-cooling-advice-2d-vs-3d-comparison-1920x1080-02-finding-7.png |
| F-008 | S4 | 3D | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/403-3d-cooling-whatif-layout-preview-1366x768-04-finding-8.png |
| F-009 | S4 | 3D | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/406-3d-cooling-whatif-layout-preview-1920-1920x1080-04-finding-9.png |
| F-010 | S4 | 3D | 1280x688 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/112-3d-heat-panel-row-and-advice-1280x688-04-finding-10.png |
| F-011 | S3 | Heat | 1366x768 | `Cannot simulate` not on screen | shots/16176-heat-refuses-20m-enclosure-1366x768-02-finding-11.png |
| F-012 | S3 | Heat | 1920x1080 | `Cannot simulate` not on screen | shots/16176-heat-refuses-20m-enclosure-1920x1080-02-finding-12.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `181-legend-with-tall-card` at 1366x768. Kind: layout-maybe.

Steps:

1. `click_text "Learn…"`
2. `wait 15`
3. `text cabinet layout`
4. `wait 8`
5. `click_text Start #1`
6. `wait 40`
7. `click_text Cabinet #1`
8. `wait 40`
9. `key shift+h`
10. `wait 60`
11. `key ctrl+shift+p`
12. `wait 40`
13. `key Enter`
14. `wait 60`
15. `shot legend-at-run-size`
16. `check_blank legend-at-run-size`
17. `expect "Preview, not a solver result"`
18. `resize 1280 688`
19. `wait 60`
20. `shot legend-squeezed`
21. `check_blank legend-squeezed`
22. `expect "Total loss"`
23. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet as table' over 'Undo'

![F-001](shots/181-legend-with-tall-card-1366x768-05-finding-1.png)

### F-002 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `181-legend-with-tall-card` at 1920x1080. Kind: layout-maybe.

Steps:

1. `click_text "Learn…"`
2. `wait 15`
3. `text cabinet layout`
4. `wait 8`
5. `click_text Start #1`
6. `wait 40`
7. `click_text Cabinet #1`
8. `wait 40`
9. `key shift+h`
10. `wait 60`
11. `key ctrl+shift+p`
12. `wait 40`
13. `key Enter`
14. `wait 60`
15. `shot legend-at-run-size`
16. `check_blank legend-at-run-size`
17. `expect "Preview, not a solver result"`
18. `resize 1280 688`
19. `wait 60`
20. `shot legend-squeezed`
21. `check_blank legend-squeezed`
22. `expect "Total loss"`
23. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet as table' over 'Undo'

![F-002](shots/181-legend-with-tall-card-1920x1080-05-finding-2.png)

### F-003 S3 `Work cards out of date` not on screen

Scenario `230-workpack-out-of-date-after-cooler-layout` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-cooler-layout.novo`
2. `wait 30`
3. `click_text 3D #1`
4. `wait 40`
5. `click_text "Layers"`
6. `wait 8`
7. `click_text "Build order"`
8. `wait 8`
9. `key Escape`
10. `wait 30`
11. `shot before-print`
12. `answer_dialog "~/Documents/KoperCAD/qa 230 work cards.pdf"`
13. `click_text "Work cards (PDF)"`
14. `wait 40`
15. `expect_file "~/Documents/KoperCAD/qa 230 work cards.pdf" 3000`
16. `shot printed`
17. `click_text Heat #1`
18. `wait 20`
19. `click_text Cancel`
20. `wait 10`
21. `click_text Auto #1`
22. `wait 10`
23. `wait 30`
24. `try click_text Cancel`
25. `wait 30`
26. `try click_text Cancel`
27. `wait 20`
28. `click_text "Settings for the next run"`
29. `wait 10`
30. `click_text Quick`
31. `wait 10`
32. `click_text Simulate #2`
33. `wait_s 120`
34. `click_text "Cooler layout"`
35. `wait_s 120`
36. `expect "Apply the moves"`
37. `click_text "Apply the moves"`
38. `wait 30`
39. `shot applied`
40. `click_text 3D #1`
41. `wait 30`
42. `shot stale`
43. `expect "Work cards out of date"`

- Expected: a control or text containing `Work cards out of date`
- Saw: not found in the widget tree

![F-003](shots/230-workpack-out-of-date-after-cooler-layout-1366x768-05-finding-3.png)

### F-004 S3 `Work cards out of date` not on screen

Scenario `230-workpack-out-of-date-after-cooler-layout` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-cooler-layout.novo`
2. `wait 30`
3. `click_text 3D #1`
4. `wait 40`
5. `click_text "Layers"`
6. `wait 8`
7. `click_text "Build order"`
8. `wait 8`
9. `key Escape`
10. `wait 30`
11. `shot before-print`
12. `answer_dialog "~/Documents/KoperCAD/qa 230 work cards.pdf"`
13. `click_text "Work cards (PDF)"`
14. `wait 40`
15. `expect_file "~/Documents/KoperCAD/qa 230 work cards.pdf" 3000`
16. `shot printed`
17. `click_text Heat #1`
18. `wait 20`
19. `click_text Cancel`
20. `wait 10`
21. `click_text Auto #1`
22. `wait 10`
23. `wait 30`
24. `try click_text Cancel`
25. `wait 30`
26. `try click_text Cancel`
27. `wait 20`
28. `click_text "Settings for the next run"`
29. `wait 10`
30. `click_text Quick`
31. `wait 10`
32. `click_text Simulate #2`
33. `wait_s 120`
34. `click_text "Cooler layout"`
35. `wait_s 120`
36. `expect "Apply the moves"`
37. `click_text "Apply the moves"`
38. `wait 30`
39. `shot applied`
40. `click_text 3D #1`
41. `wait 30`
42. `shot stale`
43. `expect "Work cards out of date"`
44. `check_layout`
45. `click_text Reports`
46. `wait 5`
47. `click_text "Bench output..."`
48. `wait 20`
49. `shot bench-stale`
50. `expect "Work cards out of date"`

- Expected: a control or text containing `Work cards out of date`
- Saw: not found in the widget tree

![F-004](shots/230-workpack-out-of-date-after-cooler-layout-1366x768-07-finding-4.png)

### F-005 S3 step failed: click_text Quick

Scenario `270-3d-heat-strip-two-layer-rows-short-window` at 1280x688. Kind: step.

Steps:

1. `open qa/fixtures/cabinet-cooling-advice.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 20`
16. `click_text "Settings for the next run"`
17. `wait 10`
18. `click_text Quick`

- Expected: the step runs
- Saw: no enabled widget labelled `Quick` (match 1)

![F-005](shots/270-3d-heat-strip-two-layer-rows-short-window-1280x688-01-finding-5.png)

### F-006 S3 `Model comparison` not on screen

Scenario `380-3d-cooling-advice-2d-vs-3d-comparison` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-cooling-advice.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 20`
16. `click_text "Settings for the next run"`
17. `wait 10`
18. `click_text Quick`
19. `wait 10`
20. `click_text Simulate #2`
21. `wait_s 60`
22. `click_text "Cooling advice"`
23. `wait_s 90`
24. `expect "Tightest margin"`
25. `click_text "Add these two to Vents and fans"`
26. `wait 10`
27. `expect "Added"`
28. `shot added-before-3d-result`
29. `click_text Simulate #2`
30. `wait_s 120`
31. `expect "Model comparison"`

- Expected: a control or text containing `Model comparison`
- Saw: not found in the widget tree

![F-006](shots/380-3d-cooling-advice-2d-vs-3d-comparison-1366x768-02-finding-6.png)

### F-007 S3 `Model comparison` not on screen

Scenario `380-3d-cooling-advice-2d-vs-3d-comparison` at 1920x1080. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-cooling-advice.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 20`
16. `click_text "Settings for the next run"`
17. `wait 10`
18. `click_text Quick`
19. `wait 10`
20. `click_text Simulate #2`
21. `wait_s 60`
22. `click_text "Cooling advice"`
23. `wait_s 90`
24. `expect "Tightest margin"`
25. `click_text "Add these two to Vents and fans"`
26. `wait 10`
27. `expect "Added"`
28. `shot added-before-3d-result`
29. `click_text Simulate #2`
30. `wait_s 120`
31. `expect "Model comparison"`

- Expected: a control or text containing `Model comparison`
- Saw: not found in the widget tree

![F-007](shots/380-3d-cooling-advice-2d-vs-3d-comparison-1920x1080-02-finding-7.png)

### F-008 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `403-3d-cooling-whatif-layout-preview` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cabinet-whatif-layout.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 20`
16. `click_text "Cooling what-if"`
17. `wait 20`
18. `click_text "Try the fixes"`
19. `wait_s 120`
20. `expect "Devices over limit"`
21. `expect "Model estimate"`
22. `shot ranked`
23. `scroll 250 420 -600`
24. `wait 10`
25. `shot ranked-scrolled`
26. `click_text "to cooler places"`
27. `wait 20`
28. `expect "Move: T1"`
29. `shot layout-move-previewed`
30. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Hide' over '4. Move 2 devices to cooler places'

![F-008](shots/403-3d-cooling-whatif-layout-preview-1366x768-04-finding-8.png)

### F-009 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `406-3d-cooling-whatif-layout-preview-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cabinet-whatif-layout.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 20`
16. `click_text "Cooling what-if"`
17. `wait 20`
18. `click_text "Try the fixes"`
19. `wait_s 120`
20. `expect "Devices over limit"`
21. `expect "Model estimate"`
22. `shot ranked`
23. `scroll 250 750 -900`
24. `wait 10`
25. `shot ranked-scrolled`
26. `click_text "to cooler places"`
27. `wait 20`
28. `expect "Move: T1"`
29. `shot layout-move-previewed`
30. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Try again with these' over '2. Add: Vent in, vent out (no power)'

![F-009](shots/406-3d-cooling-whatif-layout-preview-1920-1920x1080-04-finding-9.png)

### F-010 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `112-3d-heat-panel-row-and-advice` at 1280x688. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cabinet-cooling-advice.novo`
2. `wait 20`
3. `click_text 3D #1`
4. `wait 30`
5. `click_text Heat #1`
6. `wait 20`
7. `click_text Cancel`
8. `wait 10`
9. `click_text Auto #1`
10. `wait 10`
11. `wait 30`
12. `try click_text Cancel`
13. `wait 30`
14. `try click_text Cancel`
15. `wait 20`
16. `click_text "Settings for the next run"`
17. `wait 10`
18. `click_text Quick`
19. `wait 10`
20. `click_text Simulate #2`
21. `wait_s 45`
22. `shot heat-panel-after-run`
23. `check_layout`
24. `click_text "Cooling advice"`
25. `wait_s 90`
26. `expect "Cooling advice"`
27. `expect "Tightest margin"`
28. `shot advice-top`
29. `scroll 600 400 -400`
30. `wait 10`
31. `shot advice-bottom`
32. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '✔ Heat done' over 'Add these two to Vents and fans'

![F-010](shots/112-3d-heat-panel-row-and-advice-1280x688-04-finding-10.png)

### F-011 S3 `Cannot simulate` not on screen

Scenario `16176-heat-refuses-20m-enclosure` at 1366x768. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-huge-enclosure.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 30`
5. `key ctrl+k`
6. `wait 3`
7. `text Simulate heat of the cabinet`
8. `wait 4`
9. `key Enter`
10. `wait 30`
11. `shot refusal`
12. `expect "Cannot simulate"`

- Expected: a control or text containing `Cannot simulate`
- Saw: not found in the widget tree

![F-011](shots/16176-heat-refuses-20m-enclosure-1366x768-02-finding-11.png)

### F-012 S3 `Cannot simulate` not on screen

Scenario `16176-heat-refuses-20m-enclosure` at 1920x1080. Kind: expect.

Steps:

1. `open qa/fixtures/cabinet-huge-enclosure.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 30`
5. `key ctrl+k`
6. `wait 3`
7. `text Simulate heat of the cabinet`
8. `wait 4`
9. `key Enter`
10. `wait 30`
11. `shot refusal`
12. `expect "Cannot simulate"`

- Expected: a control or text containing `Cannot simulate`
- Saw: not found in the widget tree

![F-012](shots/16176-heat-refuses-20m-enclosure-1920x1080-02-finding-12.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
