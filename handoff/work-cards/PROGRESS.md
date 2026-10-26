2026-10-25 00:49 UTC thread started from creative audit round 34 idea 1.
2026-10-25 01:30 UTC engine, PDF, app wiring, tests (layout, pdf, workflow, app all green), QA scenarios 183-187 pass at 1366x768 and 1920x1080, light theme in 185. Reports in /mnt/project-files/qa/workpack/final.
2026-10-25 01:50 UTC review fixes: tick-off rows wrap instead of cut, '42 steps on 20 cards', cut-list fallback visible (~ and cover note) with a test, rail sort total_cmp. QA rerun from the committed head, HEAD.txt in qa/workpack/final.
2026-10-25 02:05 UTC R3: cut-list wires with no cabinet end named on the cover (test cut_list_wires_without_a_cabinet_end_are_named_on_the_cover); QA rerun.
2026-10-25 02:50 UTC #1471 merged. PR 2: Print this step, leftover-wire note matches by contains and counts per cabinet.
2026-10-25 03:40 UTC #1478 merged. PR 3: vents and fans as CUT-OUT cards (add_opening_cards, app glue from heat.openings), one-card file name per cabinet, scenario 189.
2026-10-25 05:50 UTC PR B: Per end column on the wire cards.
2026-10-25 05:10 UTC PR A: wiring openings as cut-out cards, label overlap fix.
2026-10-25 15:00 UTC #1494, #1499, #1500, #1507, #1508, #1509, #1512 merged: bench-walk findings 1 to 8 are done or decided (6 to 8 decided by the thread, see HANDOFF).
2026-10-26 05:30 UTC PR (creative round 36): revision and print date on every card and the cover; CHANGED banner on cards that differ from the last print of the session; "Work cards out of date" line in the Bench output dialog and the 3D Build order strip; cut-out sizes round to 0.5 mm; a card file can never replace the open project file. Scenario 191, reports in /mnt/project-files/qa/workpack-stamp.
2026-10-26 05:45 UTC review delta: Workshop pack print is recorded (test), saw sizes round UP to 0.5 mm, scenario renumbered 191 -> 230. QA rerun waits for #1516 and a main merge.
