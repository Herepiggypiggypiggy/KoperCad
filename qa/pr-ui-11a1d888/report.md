# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 63 run(s): 59 passed, 4 failed; 11 finding(s) (4 blocking, S1 to S3).

Program: `/tmp/kc-target2/debug/kopercad`, built 1793727124 (unix time), source at git 11a1d88873.

App: 1366.0x768.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Autopilot groups its list by protective device and follows the project's wire rules (08-autopilot-groups) | Reports | 1366x768 | pass | F-001, F-002 | 6 |
| Autopilot groups its list by protective device and follows the project's wire rules (08-autopilot-groups) | Reports | 1920x1080 | pass | F-003, F-004 | 6 |
| Autopilot fills in the example, Ready to build meter (08-autopilot) | Reports | 1366x768 | pass | - | 4 |
| Autopilot fills in the example, Ready to build meter (08-autopilot) | Reports | 1920x1080 | pass | - | 4 |
| Double click on a title bar maximises a window and a second one restores it (Pages flyout, Preferences) (1110-maximise-by-double-click) | Windows | 1366x768 | pass | - | 6 |
| Double click on a title bar maximises a window and a second one restores it (Preferences, 1920x1080) (1111-maximise-by-double-click-1920) | Windows | 1920x1080 | pass | F-005, F-006 | 5 |
| Cabinet plate: EMC view with the magnetic field estimate (25 A drive, three phases, PLC beside it) (150-emc-field-plate) | Cabinet | 1366x768 | pass | - | 3 |
| Cabinet plate (1920x1080): EMC view with the magnetic field estimate (25 A drive, three phases, PLC beside it) (150b-emc-field-plate-1920) | Cabinet | 1920x1080 | pass | - | 3 |
| Schematic: EMC net classes (wire colours, tags, legend, wire card, set class), dark and light (154-emc-net-classes-schematic) | Schematic | 1366x768 | pass | - | 6 |
| Schematic (1920x1080): EMC net classes (wire colours, tags, legend, wire card, set class), dark and light (155-emc-net-classes-schematic-1920) | Schematic | 1920x1080 | pass | - | 6 |
| Cabinet inspector: the plate card closes with its title-bar button (1366x768) (1585-inspector-close-button) | Cabinet | 1366x768 | pass | - | 2 |
| Cabinet inspector: the plate card closes with its title-bar button (1920x1080) (1586-inspector-close-button-1920) | Cabinet | 1920x1080 | pass | - | 2 |
| Several pages in the navigator on a small window: Ctrl and Shift click, drag as a block (Pages flyout) (1603-outline-multi-select-flyout) | Schematic | 1366x768 | pass | - | 7 |
| Ask Claude: the agree box stays off until the notice has been scrolled into view (17-ask-claude-agree-gate) | Assistant | 1366x600 | fail | F-007, F-008 | 4 |
| Inspector on the contactor: no trip curve, breaking capacity or let-through rows (18-contactor-inputs) | Schematic | 1366x768 | pass | - | 1 |
| Inspector on the contactor: AC-3 current shows the part's 9 A, empty rows read e.g. (19-contactor-ac3-input) | Schematic | 1366x768 | pass | - | 1 |
| Place macro: a click on the macro's name picks it (21-acceptance-macro-place) | Schematic | 1366x768 | pass | - | 2 |
| Import an I/O list (PLC cards and pages) (21-plc-io-list) | Fixtures | 1366x768 | pass | - | 7 |
| Import an I/O list (PLC cards and pages) (21-plc-io-list) | Fixtures | 1920x1080 | pass | - | 7 |
| I/O list import: the card pages get the next page numbers (no 51 new 1) (21-plc-io-page-names) | Fixtures | 1366x768 | pass | - | 1 |
| I/O list import: the card pages get the next page numbers (no 51 new 1) (21-plc-io-page-names) | Fixtures | 1920x1080 | pass | - | 1 |
| Place macro: a click on the macro's name picks it (1920x1080) (21b-acceptance-macro-place-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Challenge lesson repaired by hand ends on Solved (1366x768) (22-learn-challenge-solved) | Learn | 1366x768 | pass | - | 3 |
| Place macro from a generated page: it lands on the first drawing page, not on the contents table (223-macro-on-generated-page-goes-to-drawing) | Schematic | 1366x768 | pass | - | 4 |
| Place macro from a generated page, wide window: it lands on the first drawing page (224-macro-on-generated-page-1920) | Schematic | 1920x1080 | pass | - | 3 |
| Kitting: bought bars say to buy, rows read cleanly, and the pick list numbers are explained (226-kitting-pick-list-wording) | Reports | 1366x768 | pass | - | 1 |
| Kitting: bought bars say to buy, rows read cleanly, and the pick list numbers are explained (226-kitting-pick-list-wording) | Reports | 1920x1080 | pass | - | 1 |
| Challenge lesson repaired by hand ends on Solved (1920x1080) (23-learn-challenge-solved-1920) | Learn | 1920x1080 | pass | - | 3 |
| Recovering a damaged project can be found (37-recover-discoverable) | Start | 1366x768 | pass | - | 5 |
| Recovering a damaged project can be found (37-recover-discoverable) | Start | 1920x1080 | pass | - | 5 |
| Machine options: the documents follow the switch (43-machine-options) | Options | 1366x768 | pass | - | 6 |
| Machine options: the documents follow the switch (43-machine-options) | Options | 1920x1080 | pass | - | 6 |
| I/O list with Siemens analog addresses PIW256 and PQW256 (RM-05) (61-plc-siemens-analog) | Fixtures | 1366x768 | pass | - | 2 |
| I/O list with Siemens analog addresses PIW256 and PQW256 (RM-05) (61-plc-siemens-analog) | Fixtures | 1920x1080 | pass | - | 2 |
| Potential chip menu works with the feed low on the canvas (RM-12) (61-potential-chip-low) | Schematic | 1366x768 | pass | - | 4 |
| Potential chip menu works with the feed low on the canvas (RM-12) (61b-potential-chip-low-1920) | Schematic | 1920x1080 | pass | - | 4 |
| Standards findings are labelled in the Issues list (B-12) (62-standards-pill-note) | Reports | 1366x768 | pass | - | 2 |
| Standards findings are labelled in the Issues list (B-12) (62-standards-pill-note) | Reports | 1920x1080 | pass | - | 2 |
| Part picker takes a part number typed straight after Choose (621-part-picker-takes-typing) | Parts | 1366x768 | pass | - | 1 |
| F2 rename selects the old tag, so typing replaces it (633-f2-rename-replaces-the-tag) | Schematic | 1366x768 | pass | - | 1 |
| Ask Claude shows a Markdown answer as formatted text, not as asterisks and pipes (638-ask-claude-markdown-answer) | Assistant | 1366x768 | fail | F-009 | 1 |
| Ask Claude shows a Markdown answer as formatted text, not as asterisks and pipes (638-ask-claude-markdown-answer) | Assistant | 1920x1080 | fail | F-010 | 1 |
| Ask Claude shows a Markdown answer as formatted text, not as asterisks and pipes (638-ask-claude-markdown-answer) | Assistant | 1024x600 | fail | F-011 | 1 |
| Window sizes a rotated or mirrored monitor can report do not crash (640-window-absurd-sizes) | Display | 1366x768 | pass | - | 3 |
| Window sizes a rotated or mirrored monitor can report do not crash (640-window-absurd-sizes) | Display | 1920x1080 | pass | - | 3 |
| Import centre: tiles by purpose, search, detail pane, Import button (727-import-centre) | Layout | 1366x768 | pass | - | 4 |
| Import centre: tiles by purpose, search, detail pane, Import button (727-import-centre) | Layout | 1920x1080 | pass | - | 4 |
| A wire under way: the tool rail stays put and Select still works (736-wire-with-the-rail) | Schematic | 1366x768 | pass | - | 4 |
| PLC rows: two Draw rows, the I/O list in the Import centre, Send to PLC on right-click (7376-plc-rows) | Layout | 1366x768 | pass | - | 3 |
| PLC rows: two Draw rows, the I/O list in the Import centre, Send to PLC on right-click (7377-plc-rows-1920) | Layout | 1920x1080 | pass | - | 3 |
| Help popups open: the ? in Standards check opens its text and it fits the window (7391-help-popups-open) | Windows | 1366x768 | pass | - | 1 |
| Help popups open: the ? in Standards check opens its text and it fits the window (7391b-help-popups-open-1920) | Windows | 1920x1080 | pass | - | 1 |
| A wire in progress is cancelled by Esc or right click, and the hint says so (790-wire-cancel-by-esc-and-right-click) | Schematic | 1366x768 | pass | - | 4 |
| A wire in progress is cancelled by Esc or right click, and the hint says so (1920x1080) (790b-wire-cancel-by-esc-and-right-click-1920) | Schematic | 1920x1080 | pass | - | 4 |
| Double-clicking a tag label next to a pin renames, not wires (794-double-click-tag-renames) | Schematic | 1366x768 | pass | - | 2 |
| Double-clicking a tag label next to a pin renames, not wires (1920x1080) (794b-double-click-tag-renames-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Clicking the tip of an interruption arrow wires its pin (795-interruption-tip-connects) | Schematic | 1366x768 | pass | - | 2 |
| Clicking the tip of an interruption arrow wires its pin (1920x1080) (795b-interruption-tip-connects-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Parts table import at 700x420: the dialog fits the window and scrolls sideways (9201-parts-table-small-window) | Parts | 1366x768 | pass | - | 1 |
| Parts table import at 700x420: the dialog fits the window and scrolls sideways (9201-parts-table-small-window) | Parts | 1920x1080 | pass | - | 1 |
| Place macro: the PLC input card says it is symbols only and where to get a card (99-plc-macro-points-to-card) | Schematic | 1366x768 | pass | - | 2 |
| Place macro: the PLC input card says it is symbols only and where to get a card (1920x1080) (99b-plc-macro-points-to-card-1920) | Schematic | 1920x1080 | pass | - | 2 |
| Rename terminals opens docked right, folds, and its close button with text typed cancels (1366x768) (1146-terminal-edit-docks-folds-and-cancels) | Windows | 1366x768 | pass | - | 5 |

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S4 | Reports | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/08-autopilot-groups-1366x768-04-finding-1.png |
| F-002 | S4 | Reports | 1366x768 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/08-autopilot-groups-1366x768-06-finding-2.png |
| F-003 | S4 | Reports | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/08-autopilot-groups-1920x1080-04-finding-3.png |
| F-004 | S4 | Reports | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/08-autopilot-groups-1920x1080-06-finding-4.png |
| F-005 | S4 | Windows | 1920x1080 | 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1111-maximise-by-double-click-1920-1920x1080-03-finding-5.png |
| F-006 | S4 | Windows | 1920x1080 | 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/1111-maximise-by-double-click-1920-1920x1080-05-finding-6.png |
| F-007 | S3 | Assistant | 1366x600 | `I have read this and agree` is disabled | shots/17-ask-claude-agree-gate-1366x600-02-finding-7.png |
| F-008 | S4 | Assistant | 1366x600 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/17-ask-claude-agree-gate-1366x600-04-finding-8.png |
| F-009 | S3 | Assistant | 1366x768 | step failed: click_text "I have read this and agree" | shots/638-ask-claude-markdown-answer-1366x768-01-finding-9.png |
| F-010 | S3 | Assistant | 1920x1080 | step failed: click_text "I have read this and agree" | shots/638-ask-claude-markdown-answer-1920x1080-01-finding-10.png |
| F-011 | S3 | Assistant | 1024x600 | step failed: click_text "I have read this and agree" | shots/638-ask-claude-markdown-answer-1024x600-01-finding-11.png |

### F-001 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `08-autopilot-groups` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `click_text Autopilot`
8. `wait 20`
9. `expect "Accept all"`
10. `expect "Wire rules: IEC 60204-1"`
11. `expect "changes"`
12. `shot autopilot-groups-closed`
13. `check_blank autopilot-groups-closed`
14. `check_layout`
15. `click_text "Wire rules: IEC 60204-1"`
16. `wait 5`
17. `click_text "House rules"`
18. `wait 10`
19. `expect "Wire rules: House rules"`
20. `expect "AC control"`
21. `expect "Smallest control wire"`
22. `shot autopilot-wire-rules`
23. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'choose a size' over 'IEC 60204-1'

![F-001](shots/08-autopilot-groups-1366x768-04-finding-1.png)

### F-002 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `08-autopilot-groups` at 1366x768. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `click_text Autopilot`
8. `wait 20`
9. `expect "Accept all"`
10. `expect "Wire rules: IEC 60204-1"`
11. `expect "changes"`
12. `shot autopilot-groups-closed`
13. `check_blank autopilot-groups-closed`
14. `check_layout`
15. `click_text "Wire rules: IEC 60204-1"`
16. `wait 5`
17. `click_text "House rules"`
18. `wait 10`
19. `expect "Wire rules: House rules"`
20. `expect "AC control"`
21. `expect "Smallest control wire"`
22. `shot autopilot-wire-rules`
23. `check_layout`
24. `click_text "Wire rules: House rules"`
25. `wait 5`
26. `try scroll 1200 600 -600`
27. `try scroll 1750 600 -600`
28. `wait 4`
29. `click_text "14 changes"`
30. `wait 5`
31. `expect "Accept ticked in this group"`
32. `shot autopilot-group-open`
33. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Export bench cut list (PDF)' over 'Accept all except assumed (42)'

![F-002](shots/08-autopilot-groups-1366x768-06-finding-2.png)

### F-003 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `08-autopilot-groups` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `click_text Autopilot`
8. `wait 20`
9. `expect "Accept all"`
10. `expect "Wire rules: IEC 60204-1"`
11. `expect "changes"`
12. `shot autopilot-groups-closed`
13. `check_blank autopilot-groups-closed`
14. `check_layout`
15. `click_text "Wire rules: IEC 60204-1"`
16. `wait 5`
17. `click_text "House rules"`
18. `wait 10`
19. `expect "Wire rules: House rules"`
20. `expect "AC control"`
21. `expect "Smallest control wire"`
22. `shot autopilot-wire-rules`
23. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'choose a size' over 'IEC 60204-1'

![F-003](shots/08-autopilot-groups-1920x1080-04-finding-3.png)

### F-004 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `08-autopilot-groups` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `click_text Autopilot`
8. `wait 20`
9. `expect "Accept all"`
10. `expect "Wire rules: IEC 60204-1"`
11. `expect "changes"`
12. `shot autopilot-groups-closed`
13. `check_blank autopilot-groups-closed`
14. `check_layout`
15. `click_text "Wire rules: IEC 60204-1"`
16. `wait 5`
17. `click_text "House rules"`
18. `wait 10`
19. `expect "Wire rules: House rules"`
20. `expect "AC control"`
21. `expect "Smallest control wire"`
22. `shot autopilot-wire-rules`
23. `check_layout`
24. `click_text "Wire rules: House rules"`
25. `wait 5`
26. `try scroll 1200 600 -600`
27. `try scroll 1750 600 -600`
28. `wait 4`
29. `click_text "14 changes"`
30. `wait 5`
31. `expect "Accept ticked in this group"`
32. `shot autopilot-group-open`
33. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Export bench cut list (PDF)' over 'Accept all except assumed (42)'

![F-004](shots/08-autopilot-groups-1920x1080-06-finding-4.png)

### F-005 S4 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1111-maximise-by-double-click-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text File #1`
4. `wait 5`
5. `click_text Preferences`
6. `wait 20`
7. `click 1875 72`
8. `wait 30`
9. `shot preferences-home`
10. `double_click 960 269`
11. `wait 20`
12. `expect "Preferences"`
13. `shot preferences-maximised`
14. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Help'; 'Drawn wires (27)' over 'Help'; 'Cables (1)' over '30 s'

![F-005](shots/1111-maximise-by-double-click-1920-1920x1080-03-finding-5.png)

### F-006 S4 3 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `1111-maximise-by-double-click-1920` at 1920x1080. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text File #1`
4. `wait 5`
5. `click_text Preferences`
6. `wait 20`
7. `click 1875 72`
8. `wait 30`
9. `shot preferences-home`
10. `double_click 960 269`
11. `wait 20`
12. `expect "Preferences"`
13. `shot preferences-maximised`
14. `check_layout`
15. `double_click 960 76`
16. `wait 20`
17. `expect "Preferences"`
18. `shot preferences-restored`
19. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: ' New' over 'Help'; 'Drawn wires (27)' over 'Help'; 'Cables (1)' over '30 s'

![F-006](shots/1111-maximise-by-double-click-1920-1920x1080-05-finding-6.png)

### F-007 S3 `I have read this and agree` is disabled

Scenario `17-ask-claude-agree-gate` at 1366x600. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text ask claude`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `expect "Scroll down and read"`
10. `expect_disabled "I have read this and agree"`
11. `expect_disabled "Turn Claude on"`
12. `shot ask-claude-agree-gate-off`
13. `scroll 1200 300 -3000`
14. `wait 6`
15. `expect "4. Agree"`
16. `expect_enabled "I have read this and agree"`

- Expected: `I have read this and agree` is enabled
- Saw: the widget tree says disabled

![F-007](shots/17-ask-claude-agree-gate-1366x600-02-finding-7.png)

### F-008 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

Scenario `17-ask-claude-agree-gate` at 1366x600. Kind: layout-maybe.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text ask claude`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `expect "Scroll down and read"`
10. `expect_disabled "I have read this and agree"`
11. `expect_disabled "Turn Claude on"`
12. `shot ask-claude-agree-gate-off`
13. `scroll 1200 300 -3000`
14. `wait 6`
15. `expect "4. Agree"`
16. `expect_enabled "I have read this and agree"`
17. `shot ask-claude-agree-gate-on`
18. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: 'Terminals' over 'Expand'; 'Cables' over 'Expand'

![F-008](shots/17-ask-claude-agree-gate-1366x600-04-finding-8.png)

### F-009 S3 step failed: click_text "I have read this and agree"

Scenario `638-ask-claude-markdown-answer` at 1366x768. Kind: step.

Steps:

1. `click_text "New project"`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text ask claude`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `try scroll 1200 400 -300`
10. `try scroll 1750 400 -300`
11. `try scroll 880 400 -300`
12. `wait 4`
13. `click_text "Whole project"`
14. `wait 4`
15. `try scroll 1200 400 -3000`
16. `try scroll 1750 400 -3000`
17. `try scroll 880 400 -3000`
18. `wait 4`
19. `click_text "I have read this and agree"`

- Expected: the step runs
- Saw: no enabled widget labelled `I have read this and agree` (match 1)

![F-009](shots/638-ask-claude-markdown-answer-1366x768-01-finding-9.png)

### F-010 S3 step failed: click_text "I have read this and agree"

Scenario `638-ask-claude-markdown-answer` at 1920x1080. Kind: step.

Steps:

1. `click_text "New project"`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text ask claude`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `try scroll 1200 400 -300`
10. `try scroll 1750 400 -300`
11. `try scroll 880 400 -300`
12. `wait 4`
13. `click_text "Whole project"`
14. `wait 4`
15. `try scroll 1200 400 -3000`
16. `try scroll 1750 400 -3000`
17. `try scroll 880 400 -3000`
18. `wait 4`
19. `click_text "I have read this and agree"`

- Expected: the step runs
- Saw: no enabled widget labelled `I have read this and agree` (match 1)

![F-010](shots/638-ask-claude-markdown-answer-1920x1080-01-finding-10.png)

### F-011 S3 step failed: click_text "I have read this and agree"

Scenario `638-ask-claude-markdown-answer` at 1024x600. Kind: step.

Steps:

1. `click_text "New project"`
2. `wait 20`
3. `key ctrl+k`
4. `wait 6`
5. `text ask claude`
6. `wait 8`
7. `key Enter`
8. `wait 10`
9. `try scroll 1200 400 -300`
10. `try scroll 1750 400 -300`
11. `try scroll 880 400 -300`
12. `wait 4`
13. `click_text "Whole project"`
14. `wait 4`
15. `try scroll 1200 400 -3000`
16. `try scroll 1750 400 -3000`
17. `try scroll 880 400 -3000`
18. `wait 4`
19. `click_text "I have read this and agree"`

- Expected: the step runs
- Saw: no enabled widget labelled `I have read this and agree` (match 1)

![F-011](shots/638-ask-claude-markdown-answer-1024x600-01-finding-11.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
