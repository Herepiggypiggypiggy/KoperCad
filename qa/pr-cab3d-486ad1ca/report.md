# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 23 run(s): 23 passed, 0 failed; 12 finding(s) (0 blocking, S1 to S3).

Program: `/tmp/kc-target3/debug/kopercad`, built 1793726900 (unix time), source at git 486ad1ca67.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Cabinet: the Arrange card, previews with their notes, Cancel stores nothing, Accept is one undo step (1366x768) (1720-cabinet-arrange-tools-1366) | Cabinet | 1366x768 | pass | - | 15 |
| Cable runs in 3D: Wire runs list, Run card, lit path (31-cable-runs-3d) | 3D | 1366x768 | pass | - | 4 |
| Cable runs in 3D: Wire runs list, Run card, lit path (31-cable-runs-3d) | 3D | 1920x1080 | pass | - | 4 |
| Duct runs: pick a duct in 3D, read the runs in it (1920x1080) (35-duct-runs-1920) | 3D | 1920x1080 | pass | - | 2 |
| 3D view: front view, select a device, dimensions and clearance zone (41-3d-select-clearance) | 3D | 1366x768 | pass | - | 1 |
| Schematic: EMC cable card previews its fix, Light theme, and Cancel changes nothing (1366x768) (420-emc-fixit-cable-card-preview-light) | Schematic | 1366x768 | pass | - | 2 |
| Schematic: EMC cable card previews its fix, Light theme, and Cancel changes nothing (1920x1080) (421-emc-fixit-cable-card-preview-light-1920) | Schematic | 1920x1080 | pass | - | 2 |
| 3D view: select a device, nudge, rotate, undo (44-3d-edit-device) | 3D | 1366x768 | pass | - | 4 |
| EMC fix-it: move a sensitive device out of the field of a wire, preview and apply (470-emc-fixit-move-out-of-field) | Cabinet | 1366x768 | pass | F-001, F-002 | 6 |
| EMC fix-it: move a sensitive device out of the field of a wire, preview and apply (470-emc-fixit-move-out-of-field) | Cabinet | 1920x1080 | pass | F-003, F-004 | 6 |
| EMC fix-it: the move-out-of-field preview in the Light theme (471-emc-fixit-move-out-of-field-light) | Cabinet | 1366x768 | pass | F-005, F-006 | 6 |
| EMC fix-it: the move-out-of-field preview in the Light theme (471-emc-fixit-move-out-of-field-light) | Cabinet | 1920x1080 | pass | F-007, F-008 | 6 |
| 3D view: select a device and duplicate it, then undo (48-3d-duplicate) | 3D | 1366x768 | pass | - | 2 |
| 3D view: type a position, then drag a device across the plate, undo both (49-3d-position-drag) | 3D | 1366x768 | pass | - | 3 |
| Cross-cabinet run: the Run card names the other cabinet (1366x768) (55-cable-cross-cabinet) | 3D | 1366x768 | pass | F-009 | 4 |
| Cross-cabinet run: the Run card names the other cabinet (1920x1080) (56-cable-cross-cabinet-1920) | 3D | 1920x1080 | pass | F-010 | 4 |
| 3D view: Move is off by default, so a plain drag cannot move a duct through the wall (637-3d-move-off-by-default) | 3D | 1366x768 | pass | - | 3 |
| Set location for several devices, then terminals land on strips by kind (70-set-location-and-strips-1920) | Terminals | 1920x1080 | pass | - | 5 |
| Single-line diagram: the example with stated supply data, overload relay box, Enter supply data (70-single-line-example-supply) | Reports | 1366x768 | pass | - | 3 |
| Single-line diagram: the example with stated supply data, overload relay box, Enter supply data (70-single-line-example-supply) | Reports | 1920x1080 | pass | - | 3 |
| Cabinet: a device dragged near its rail clips onto it by its centre, one dragged away leaves it (79-cabinet-rail-snap) | Cabinet | 1366x768 | pass | - | 5 |
| Cabinet (1920x1080): Centre rails on devices puts each rail through the middle of its devices without moving them (87-centre-rails-on-devices-1920) | Cabinet | 1920x1080 | pass | F-011 | 5 |
| Cabinet: Centre rails on devices puts each rail through the middle of its devices without moving them (87-centre-rails-on-devices) | Cabinet | 1366x768 | pass | F-012 | 5 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Cabinet | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/470-emc-fixit-move-out-of-field-1366x768-03-finding-1.png |
| F-002 | S4 | Cabinet | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/470-emc-fixit-move-out-of-field-1366x768-05-finding-2.png |
| F-003 | S4 | Cabinet | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/470-emc-fixit-move-out-of-field-1920x1080-03-finding-3.png |
| F-004 | S4 | Cabinet | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/470-emc-fixit-move-out-of-field-1920x1080-05-finding-4.png |
| F-005 | S4 | Cabinet | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/471-emc-fixit-move-out-of-field-light-1366x768-03-finding-5.png |
| F-006 | S4 | Cabinet | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/471-emc-fixit-move-out-of-field-light-1366x768-05-finding-6.png |
| F-007 | S4 | Cabinet | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/471-emc-fixit-move-out-of-field-light-1920x1080-03-finding-7.png |
| F-008 | S4 | Cabinet | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/471-emc-fixit-move-out-of-field-light-1920x1080-05-finding-8.png |
| F-009 | S4 | 3D | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/55-cable-cross-cabinet-1366x768-02-finding-9.png |
| F-010 | S4 | 3D | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/56-cable-cross-cabinet-1920-1920x1080-02-finding-10.png |
| F-011 | S4 | Cabinet | 1920x1080 | 4 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/87-centre-rails-on-devices-1920-1920x1080-04-finding-11.png |
| F-012 | S4 | Cabinet | 1366x768 | 5 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/87-centre-rails-on-devices-1366x768-04-finding-12.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `470-emc-fixit-move-out-of-field` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text emc check`
6. `wait 8`
7. `key Enter`
8. `wait 30`
9. `scroll 360 400 -700`
10. `wait 6`
11. `shot 00-scrolled`
12. `expect "Preview: Move it to the nearest free spot where the modelled field"`
13. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
14. `wait 10`
15. `expect "Preview on a copy of the project"`
16. `expect "modelled field there"`
17. `expect "Eases EMC-19"`
18. `expect "(one eased)"`
19. `shot 01-preview`
20. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet' over 'Close window'

![F-001](shots/470-emc-fixit-move-out-of-field-1366x768-03-finding-1.png)

### F-002 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `470-emc-fixit-move-out-of-field` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text emc check`
6. `wait 8`
7. `key Enter`
8. `wait 30`
9. `scroll 360 400 -700`
10. `wait 6`
11. `shot 00-scrolled`
12. `expect "Preview: Move it to the nearest free spot where the modelled field"`
13. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
14. `wait 10`
15. `expect "Preview on a copy of the project"`
16. `expect "modelled field there"`
17. `expect "Eases EMC-19"`
18. `expect "(one eased)"`
19. `shot 01-preview`
20. `check_layout`
21. `click_text "Apply (one undo step)" #1`
22. `wait 15`
23. `expect "New position"`
24. `expect_not "Preview on a copy of the project"`
25. `shot 02-applied`
26. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet' over 'Close window'

![F-002](shots/470-emc-fixit-move-out-of-field-1366x768-05-finding-2.png)

### F-003 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `470-emc-fixit-move-out-of-field` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text emc check`
6. `wait 8`
7. `key Enter`
8. `wait 30`
9. `scroll 360 400 -700`
10. `wait 6`
11. `shot 00-scrolled`
12. `expect "Preview: Move it to the nearest free spot where the modelled field"`
13. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
14. `wait 10`
15. `expect "Preview on a copy of the project"`
16. `expect "modelled field there"`
17. `expect "Eases EMC-19"`
18. `expect "(one eased)"`
19. `shot 01-preview`
20. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Run the EMC rules with the design checks'

![F-003](shots/470-emc-fixit-move-out-of-field-1920x1080-03-finding-3.png)

### F-004 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `470-emc-fixit-move-out-of-field` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text emc check`
6. `wait 8`
7. `key Enter`
8. `wait 30`
9. `scroll 360 400 -700`
10. `wait 6`
11. `shot 00-scrolled`
12. `expect "Preview: Move it to the nearest free spot where the modelled field"`
13. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
14. `wait 10`
15. `expect "Preview on a copy of the project"`
16. `expect "modelled field there"`
17. `expect "Eases EMC-19"`
18. `expect "(one eased)"`
19. `shot 01-preview`
20. `check_layout`
21. `click_text "Apply (one undo step)" #1`
22. `wait 15`
23. `expect "New position"`
24. `expect_not "Preview on a copy of the project"`
25. `shot 02-applied`
26. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Run the EMC rules with the design checks'

![F-004](shots/470-emc-fixit-move-out-of-field-1920x1080-05-finding-4.png)

### F-005 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `471-emc-fixit-move-out-of-field-light` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text preferences`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `click_text Light`
10. `wait 10`
11. `key Escape`
12. `wait 20`
13. `key ctrl+k`
14. `wait 6`
15. `text emc check`
16. `wait 8`
17. `key Enter`
18. `wait 30`
19. `scroll 360 400 -700`
20. `wait 6`
21. `shot 00-scrolled`
22. `expect "Preview: Move it to the nearest free spot where the modelled field"`
23. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
24. `wait 10`
25. `expect "Preview on a copy of the project"`
26. `expect "modelled field there"`
27. `expect "Eases EMC-19"`
28. `expect "(one eased)"`
29. `shot 01-preview`
30. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet' over 'Close window'

![F-005](shots/471-emc-fixit-move-out-of-field-light-1366x768-03-finding-5.png)

### F-006 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `471-emc-fixit-move-out-of-field-light` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text preferences`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `click_text Light`
10. `wait 10`
11. `key Escape`
12. `wait 20`
13. `key ctrl+k`
14. `wait 6`
15. `text emc check`
16. `wait 8`
17. `key Enter`
18. `wait 30`
19. `scroll 360 400 -700`
20. `wait 6`
21. `shot 00-scrolled`
22. `expect "Preview: Move it to the nearest free spot where the modelled field"`
23. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
24. `wait 10`
25. `expect "Preview on a copy of the project"`
26. `expect "modelled field there"`
27. `expect "Eases EMC-19"`
28. `expect "(one eased)"`
29. `shot 01-preview`
30. `check_layout`
31. `click_text "Apply (one undo step)" #1`
32. `wait 15`
33. `expect "New position"`
34. `expect_not "Preview on a copy of the project"`
35. `shot 02-applied`
36. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet' over 'Close window'

![F-006](shots/471-emc-fixit-move-out-of-field-light-1366x768-05-finding-6.png)

### F-007 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `471-emc-fixit-move-out-of-field-light` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text preferences`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `click_text Light`
10. `wait 10`
11. `key Escape`
12. `wait 20`
13. `key ctrl+k`
14. `wait 6`
15. `text emc check`
16. `wait 8`
17. `key Enter`
18. `wait 30`
19. `scroll 360 400 -700`
20. `wait 6`
21. `shot 00-scrolled`
22. `expect "Preview: Move it to the nearest free spot where the modelled field"`
23. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
24. `wait 10`
25. `expect "Preview on a copy of the project"`
26. `expect "modelled field there"`
27. `expect "Eases EMC-19"`
28. `expect "(one eased)"`
29. `shot 01-preview`
30. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Run the EMC rules with the design checks'

![F-007](shots/471-emc-fixit-move-out-of-field-light-1920x1080-03-finding-7.png)

### F-008 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `471-emc-fixit-move-out-of-field-light` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/emc-field.novo`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text preferences`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `click_text Light`
10. `wait 10`
11. `key Escape`
12. `wait 20`
13. `key ctrl+k`
14. `wait 6`
15. `text emc check`
16. `wait 8`
17. `key Enter`
18. `wait 30`
19. `scroll 360 400 -700`
20. `wait 6`
21. `shot 00-scrolled`
22. `expect "Preview: Move it to the nearest free spot where the modelled field"`
23. `click_text "Preview: Move it to the nearest free spot where the modelled field" #1`
24. `wait 10`
25. `expect "Preview on a copy of the project"`
26. `expect "modelled field there"`
27. `expect "Eases EMC-19"`
28. `expect "(one eased)"`
29. `shot 01-preview`
30. `check_layout`
31. `click_text "Apply (one undo step)" #1`
32. `wait 15`
33. `expect "New position"`
34. `expect_not "Preview on a copy of the project"`
35. `shot 02-applied`
36. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Run the EMC rules with the design checks'

![F-008](shots/471-emc-fixit-move-out-of-field-light-1920x1080-05-finding-8.png)

### F-009 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `55-cable-cross-cabinet` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cable-cross.novo`
2. `wait 20`
3. `resize 1366 768`
4. `wait 10`
5. `click_text 3D`
6. `wait 40`
7. `key ctrl+k`
8. `wait 3`
9. `text wire runs`
10. `wait 4`
11. `key Enter`
12. `wait 40`
13. `shot list`
14. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '8   886 mm' over 'Not drawn (12)'

![F-009](shots/55-cable-cross-cabinet-1366x768-02-finding-9.png)

### F-010 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `56-cable-cross-cabinet-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/cable-cross.novo`
2. `wait 20`
3. `resize 1920 1080`
4. `wait 10`
5. `click_text 3D`
6. `wait 40`
7. `key ctrl+k`
8. `wait 3`
9. `text wire runs`
10. `wait 4`
11. `key Enter`
12. `wait 40`
13. `shot list`
14. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '11   610 mm' over 'Not drawn (12)'

![F-010](shots/56-cable-cross-cabinet-1920-1920x1080-02-finding-10.png)

### F-011 S4 4 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `87-centre-rails-on-devices-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/door-cutouts.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 30`
5. `shot before`
6. `key ctrl+k`
7. `wait 3`
8. `text rails and ducts`
9. `wait 4`
10. `key Enter`
11. `wait 10`
12. `click 1875 72`
13. `wait 20`
14. `expect "Centre rails on devices"`
15. `shot window`
16. `click_text "Centre rails on devices"`
17. `wait 10`
18. `expect "devices unmoved"`
19. `shot after`
20. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Reports' over 'Close window'; ' New' over 'Lesson: Rails and wiring ducts'; '+CAB' over 'Add duct'; '  Rules' over 'Centre rails on devices'

![F-011](shots/87-centre-rails-on-devices-1920-1920x1080-04-finding-11.png)

### F-012 S4 5 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `87-centre-rails-on-devices` at 1366x768. Kind: layout-maybe.

Steps:

1. `open qa/fixtures/door-cutouts.novo`
2. `wait 20`
3. `click_text Cabinet #1`
4. `wait 30`
5. `shot before`
6. `key ctrl+k`
7. `wait 3`
8. `text rails and ducts`
9. `wait 4`
10. `key Enter`
11. `wait 10`
12. `click 1321 72`
13. `wait 20`
14. `expect "Centre rails on devices"`
15. `shot window`
16. `click_text "Centre rails on devices"`
17. `wait 10`
18. `expect "devices unmoved"`
19. `shot after`
20. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Cabinet' over 'Close window'; '+CAB' over 'Lesson: Rails and wiring ducts'; '  Rules' over 'Add duct'; '  Plate setup' over 'Add vertical duct'; '  Size enclosure' over 'Centre rails on devices'

![F-012](shots/87-centre-rails-on-devices-1366x768-04-finding-12.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
