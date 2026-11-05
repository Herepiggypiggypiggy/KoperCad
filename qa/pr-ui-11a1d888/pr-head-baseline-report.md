# KoperCAD QA run

Platform linux, window sizes 1366x768, 1920x1080. 14 run(s): 0 passed, 14 failed; 28 finding(s) (18 blocking, S1 to S3).

Program: `/tmp/kc-main/debug/kopercad`, built 1793729206 (unix time), source at git d967880afc.

App: 1366.0x600.0 points at scale 1.0.

## Scenarios

| Scenario | Area | Size | Result | Findings | Pictures |
|---|---|---|---|---|---|
| Ask Claude: the agree box stays off until the notice has been scrolled into view (17-ask-claude-agree-gate) | Assistant | 1366x600 | fail | F-001, F-002 | 4 |
| Ask Claude shows a Markdown answer as formatted text, not as asterisks and pipes (638-ask-claude-markdown-answer) | Assistant | 1366x768 | fail | F-003 | 1 |
| Ask Claude shows a Markdown answer as formatted text, not as asterisks and pipes (638-ask-claude-markdown-answer) | Assistant | 1920x1080 | fail | F-004 | 1 |
| Ask Claude shows a Markdown answer as formatted text, not as asterisks and pipes (638-ask-claude-markdown-answer) | Assistant | 1024x600 | fail | F-005 | 1 |
| Bench packet: window button, command and file (38-bench-packet) | Reports | 1366x768 | fail | F-006, F-007 | 5 |
| Bench packet: window button, command and file (38-bench-packet) | Reports | 1920x1080 | fail | F-008, F-009 | 5 |
| Print scale check: window row, advice for a measured square, command and file (39-print-scale-check) | Reports | 1366x768 | fail | F-010 | 5 |
| Print scale check: window row, advice for a measured square, command and file (39-print-scale-check) | Reports | 1920x1080 | fail | F-011 | 5 |
| Work cards: Bench output button, light theme (185-workpack-bench-output-light) | Reports | 1366x768 | fail | F-012, F-013 | 4 |
| Work cards: Bench output button, light theme (185-workpack-bench-output-light) | Reports | 1920x1080 | fail | F-014, F-015 | 4 |
| Lesson walk: wire-test (Test tab, reading, record, miss, list, PDF) (210-learn-wire-test) | Learn | 1366x768 | fail | F-016 | 14 |
| Lesson walk: wire-test (Test tab, reading, record, miss, list, PDF) (210-learn-wire-test) | Learn | 1920x1080 | fail | F-017, F-018, F-019, F-020, F-021, F-022, F-023, F-024, F-025, F-026 | 23 |
| Lesson walk by real clicks: Will the door close? (Apply, already applied, ring inside the Inspector) (284-learn-door-real-clicks) | Learn | 1366x768 | fail | F-027 | 4 |
| Lesson walk by real clicks: Will the door close? (Apply, already applied, ring inside the Inspector) (284-learn-door-real-clicks) | Learn | 1920x1080 | fail | F-028 | 4 |

## Files kept

Exports the run wrote, copied next to this report:

- `files/39-print-scale-check-1366x768-Motor_starter__example__print_scale_check.pdf`
- `files/39-print-scale-check-1920x1080-Motor_starter__example__print_scale_check.pdf`

## Findings

| # | Sev | Area | Size | Title | Evidence |
|---|---|---|---|---|---|
| F-001 | S3 | Assistant | 1366x600 | `I have read this and agree` is disabled | shots/17-ask-claude-agree-gate-1366x600-02-finding-1.png |
| F-002 | S4 | Assistant | 1366x600 | 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/17-ask-claude-agree-gate-1366x600-04-finding-2.png |
| F-003 | S3 | Assistant | 1366x768 | step failed: click_text "I have read this and agree" | shots/638-ask-claude-markdown-answer-1366x768-01-finding-3.png |
| F-004 | S3 | Assistant | 1920x1080 | step failed: click_text "I have read this and agree" | shots/638-ask-claude-markdown-answer-1920x1080-01-finding-4.png |
| F-005 | S3 | Assistant | 1024x600 | step failed: click_text "I have read this and agree" | shots/638-ask-claude-markdown-answer-1024x600-01-finding-5.png |
| F-006 | S2 | Reports | 1366x768 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing or too small | shots/38-bench-packet-1366x768-02-finding-6.png |
| F-007 | S2 | Reports | 1366x768 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing, nothing to keep | shots/38-bench-packet-1366x768-03-finding-7.png |
| F-008 | S2 | Reports | 1920x1080 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing or too small | shots/38-bench-packet-1920x1080-02-finding-8.png |
| F-009 | S2 | Reports | 1920x1080 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing, nothing to keep | shots/38-bench-packet-1920x1080-03-finding-9.png |
| F-010 | S3 | Reports | 1366x768 | `Print again at` not on screen | shots/39-print-scale-check-1366x768-03-finding-10.png |
| F-011 | S3 | Reports | 1920x1080 | `Print again at` not on screen | shots/39-print-scale-check-1920x1080-03-finding-11.png |
| F-012 | S2 | Reports | 1366x768 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing or too small | shots/185-workpack-bench-output-light-1366x768-03-finding-12.png |
| F-013 | S2 | Reports | 1366x768 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing, nothing to keep | shots/185-workpack-bench-output-light-1366x768-04-finding-13.png |
| F-014 | S2 | Reports | 1920x1080 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing or too small | shots/185-workpack-bench-output-light-1920x1080-03-finding-14.png |
| F-015 | S2 | Reports | 1920x1080 | file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing, nothing to keep | shots/185-workpack-bench-output-light-1920x1080-04-finding-15.png |
| F-016 | S3 | Learn | 1366x768 | `Recorded` not on screen | shots/210-learn-wire-test-1366x768-08-finding-16.png |
| F-017 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-05-finding-17.png |
| F-018 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-07-finding-18.png |
| F-019 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-09-finding-19.png |
| F-020 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-11-finding-20.png |
| F-021 | S3 | Learn | 1920x1080 | `Recorded` not on screen | shots/210-learn-wire-test-1920x1080-12-finding-21.png |
| F-022 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-14-finding-22.png |
| F-023 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-16-finding-23.png |
| F-024 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-19-finding-24.png |
| F-025 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-21-finding-25.png |
| F-026 | S4 | Learn | 1920x1080 | 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture) | shots/210-learn-wire-test-1920x1080-23-finding-26.png |
| F-027 | S3 | Learn | 1366x768 | step failed: click_text "Next" | shots/284-learn-door-real-clicks-1366x768-04-finding-27.png |
| F-028 | S3 | Learn | 1920x1080 | step failed: click_text "Next" | shots/284-learn-door-real-clicks-1920x1080-04-finding-28.png |

### F-001 S3 `I have read this and agree` is disabled

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

![F-001](shots/17-ask-claude-agree-gate-1366x600-02-finding-1.png)

### F-002 S4 2 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-002](shots/17-ask-claude-agree-gate-1366x600-04-finding-2.png)

### F-003 S3 step failed: click_text "I have read this and agree"

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

![F-003](shots/638-ask-claude-markdown-answer-1366x768-01-finding-3.png)

### F-004 S3 step failed: click_text "I have read this and agree"

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

![F-004](shots/638-ask-claude-markdown-answer-1920x1080-01-finding-4.png)

### F-005 S3 step failed: click_text "I have read this and agree"

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

![F-005](shots/638-ask-claude-markdown-answer-1024x600-01-finding-5.png)

### F-006 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing or too small

Scenario `38-bench-packet` at 1366x768. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `expect "Bench packet (PDF)"`
8. `shot window`
9. `check_layout`
10. `click_text "Bench packet (PDF)"`
11. `wait 30`
12. `expect_file "~/Documents/KoperCAD/*bench packet.pdf" 1500`

- Expected: the file exists and has at least 1500 bytes
- Saw: 0 bytes

![F-006](shots/38-bench-packet-1366x768-02-finding-6.png)

### F-007 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing, nothing to keep

Scenario `38-bench-packet` at 1366x768. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `expect "Bench packet (PDF)"`
8. `shot window`
9. `check_layout`
10. `click_text "Bench packet (PDF)"`
11. `wait 30`
12. `expect_file "~/Documents/KoperCAD/*bench packet.pdf" 1500`
13. `keep_file "~/Documents/KoperCAD/*bench packet.pdf"`

- Expected: the export wrote the file
- Saw: not found

![F-007](shots/38-bench-packet-1366x768-03-finding-7.png)

### F-008 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing or too small

Scenario `38-bench-packet` at 1920x1080. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `expect "Bench packet (PDF)"`
8. `shot window`
9. `check_layout`
10. `click_text "Bench packet (PDF)"`
11. `wait 30`
12. `expect_file "~/Documents/KoperCAD/*bench packet.pdf" 1500`

- Expected: the file exists and has at least 1500 bytes
- Saw: 0 bytes

![F-008](shots/38-bench-packet-1920x1080-02-finding-8.png)

### F-009 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/*bench packet.pdf` missing, nothing to keep

Scenario `38-bench-packet` at 1920x1080. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `expect "Bench packet (PDF)"`
8. `shot window`
9. `check_layout`
10. `click_text "Bench packet (PDF)"`
11. `wait 30`
12. `expect_file "~/Documents/KoperCAD/*bench packet.pdf" 1500`
13. `keep_file "~/Documents/KoperCAD/*bench packet.pdf"`

- Expected: the export wrote the file
- Saw: not found

![F-009](shots/38-bench-packet-1920x1080-03-finding-9.png)

### F-010 S3 `Print again at` not on screen

Scenario `39-print-scale-check` at 1366x768. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `expect "Print scale check (PDF)"`
8. `shot window`
9. `check_layout`
10. `click_text "Print scale check (PDF)"`
11. `wait 30`
12. `expect_file "~/Documents/KoperCAD/*print scale check.pdf" 1500`
13. `keep_file "~/Documents/KoperCAD/*print scale check.pdf"`
14. `shot written`
15. `click_text "Square width"`
16. `text 99,2`
17. `click_text "Square height"`
18. `text 99,1`
19. `wait 4`
20. `expect "Print again at"`

- Expected: a control or text containing `Print again at`
- Saw: not found in the widget tree

![F-010](shots/39-print-scale-check-1366x768-03-finding-10.png)

### F-011 S3 `Print again at` not on screen

Scenario `39-print-scale-check` at 1920x1080. Kind: expect.

Steps:

1. `open_starter_example`
2. `wait 20`
3. `click_text Reports`
4. `wait 5`
5. `click_text "Bench output..."`
6. `wait 20`
7. `expect "Print scale check (PDF)"`
8. `shot window`
9. `check_layout`
10. `click_text "Print scale check (PDF)"`
11. `wait 30`
12. `expect_file "~/Documents/KoperCAD/*print scale check.pdf" 1500`
13. `keep_file "~/Documents/KoperCAD/*print scale check.pdf"`
14. `shot written`
15. `click_text "Square width"`
16. `text 99,2`
17. `click_text "Square height"`
18. `text 99,1`
19. `wait 4`
20. `expect "Print again at"`

- Expected: a control or text containing `Print again at`
- Saw: not found in the widget tree

![F-011](shots/39-print-scale-check-1920x1080-03-finding-11.png)

### F-012 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing or too small

Scenario `185-workpack-bench-output-light` at 1366x768. Kind: expect.

Steps:

1. `open_starter_example`
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
12. `wait 6`
13. `click_text Reports`
14. `wait 5`
15. `click_text "Bench output..."`
16. `wait 20`
17. `shot bench-output-light`
18. `expect "Work cards (PDF)"`
19. `check_layout`
20. `answer_dialog "~/Documents/KoperCAD/qa 185 work cards.pdf"`
21. `click_text "Work cards (PDF)"`
22. `wait 40`
23. `shot written-light`
24. `expect_file "~/Documents/KoperCAD/qa 185 work cards.pdf" 3000`

- Expected: the file exists and has at least 3000 bytes
- Saw: 0 bytes

![F-012](shots/185-workpack-bench-output-light-1366x768-03-finding-12.png)

### F-013 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing, nothing to keep

Scenario `185-workpack-bench-output-light` at 1366x768. Kind: expect.

Steps:

1. `open_starter_example`
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
12. `wait 6`
13. `click_text Reports`
14. `wait 5`
15. `click_text "Bench output..."`
16. `wait 20`
17. `shot bench-output-light`
18. `expect "Work cards (PDF)"`
19. `check_layout`
20. `answer_dialog "~/Documents/KoperCAD/qa 185 work cards.pdf"`
21. `click_text "Work cards (PDF)"`
22. `wait 40`
23. `shot written-light`
24. `expect_file "~/Documents/KoperCAD/qa 185 work cards.pdf" 3000`
25. `keep_file "~/Documents/KoperCAD/qa 185 work cards.pdf"`

- Expected: the export wrote the file
- Saw: not found

![F-013](shots/185-workpack-bench-output-light-1366x768-04-finding-13.png)

### F-014 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing or too small

Scenario `185-workpack-bench-output-light` at 1920x1080. Kind: expect.

Steps:

1. `open_starter_example`
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
12. `wait 6`
13. `click_text Reports`
14. `wait 5`
15. `click_text "Bench output..."`
16. `wait 20`
17. `shot bench-output-light`
18. `expect "Work cards (PDF)"`
19. `check_layout`
20. `answer_dialog "~/Documents/KoperCAD/qa 185 work cards.pdf"`
21. `click_text "Work cards (PDF)"`
22. `wait 40`
23. `shot written-light`
24. `expect_file "~/Documents/KoperCAD/qa 185 work cards.pdf" 3000`

- Expected: the file exists and has at least 3000 bytes
- Saw: 0 bytes

![F-014](shots/185-workpack-bench-output-light-1920x1080-03-finding-14.png)

### F-015 S2 file `/tmp/kopercad-test-user-11049/home/Documents/KoperCAD/qa 185 work cards.pdf` missing, nothing to keep

Scenario `185-workpack-bench-output-light` at 1920x1080. Kind: expect.

Steps:

1. `open_starter_example`
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
12. `wait 6`
13. `click_text Reports`
14. `wait 5`
15. `click_text "Bench output..."`
16. `wait 20`
17. `shot bench-output-light`
18. `expect "Work cards (PDF)"`
19. `check_layout`
20. `answer_dialog "~/Documents/KoperCAD/qa 185 work cards.pdf"`
21. `click_text "Work cards (PDF)"`
22. `wait 40`
23. `shot written-light`
24. `expect_file "~/Documents/KoperCAD/qa 185 work cards.pdf" 3000`
25. `keep_file "~/Documents/KoperCAD/qa 185 work cards.pdf"`

- Expected: the export wrote the file
- Saw: not found

![F-015](shots/185-workpack-bench-output-light-1920x1080-04-finding-15.png)

### F-016 S3 `Recorded` not on screen

Scenario `210-learn-wire-test` at 1366x768. Kind: expect.

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

- Expected: a control or text containing `Recorded`
- Saw: not found in the widget tree

![F-016](shots/210-learn-wire-test-1366x768-08-finding-16.png)

### F-017 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-017](shots/210-learn-wire-test-1920x1080-05-finding-17.png)

### F-018 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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

![F-018](shots/210-learn-wire-test-1920x1080-07-finding-18.png)

### F-019 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
29. `wait 5`
30. `text 50`
31. `wait 5`
32. `shot 06-typed`
33. `check_layout`

- Expected: every control fully visible and not overlapped
- Saw: '7.  Wire wire' over 'Write test sheet PDF'

![F-019](shots/210-learn-wire-test-1920x1080-09-finding-19.png)

### F-020 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

![F-020](shots/210-learn-wire-test-1920x1080-11-finding-20.png)

### F-021 S3 `Recorded` not on screen

Scenario `210-learn-wire-test` at 1920x1080. Kind: expect.

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

- Expected: a control or text containing `Recorded`
- Saw: not found in the widget tree

![F-021](shots/210-learn-wire-test-1920x1080-12-finding-21.png)

### F-022 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

![F-022](shots/210-learn-wire-test-1920x1080-14-finding-22.png)

### F-023 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

![F-023](shots/210-learn-wire-test-1920x1080-16-finding-23.png)

### F-024 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

![F-024](shots/210-learn-wire-test-1920x1080-19-finding-24.png)

### F-025 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

![F-025](shots/210-learn-wire-test-1920x1080-21-finding-25.png)

### F-026 S4 1 pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)

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
25. `click 48 475`
26. `wait 5`
27. `text 0.12`
28. `click 48 523`
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

![F-026](shots/210-learn-wire-test-1920x1080-23-finding-26.png)

### F-027 S3 step failed: click_text "Next"

Scenario `284-learn-door-real-clicks` at 1366x768. Kind: step.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text door close`
4. `wait 6`
5. `click_text "Will the door close?"`
6. `wait 40`
7. `shot 01-start`
8. `check_layout`
9. `click_text "Cabinet" #1`
10. `wait 30`
11. `shot 02-cabinet`
12. `check_layout`
13. `click_text "Plate setup"`
14. `wait 20`
15. `shot 03-inside-and-door-ring`
16. `check_layout`
17. `expect "Inside width"`
18. `click_text "Next"`

- Expected: the step runs
- Saw: no enabled widget labelled `Next` (match 1)

![F-027](shots/284-learn-door-real-clicks-1366x768-04-finding-27.png)

### F-028 S3 step failed: click_text "Next"

Scenario `284-learn-door-real-clicks` at 1920x1080. Kind: step.

Steps:

1. `key ctrl+k`
2. `wait 4`
3. `text door close`
4. `wait 6`
5. `click_text "Will the door close?"`
6. `wait 40`
7. `shot 01-start`
8. `check_layout`
9. `click_text "Cabinet" #1`
10. `wait 30`
11. `shot 02-cabinet`
12. `check_layout`
13. `click_text "Plate setup"`
14. `wait 20`
15. `shot 03-inside-and-door-ring`
16. `check_layout`
17. `expect "Inside width"`
18. `click_text "Next"`

- Expected: the step runs
- Saw: no enabled widget labelled `Next` (match 1)

![F-028](shots/284-learn-door-real-clicks-1920x1080-04-finding-28.png)

Severity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility.
