2026-10-22 20:22 UTC SCALE-2 started from the project lead's list
2026-10-22 21:30 UTC measured engine (all fast) and window at 600 pages; found Commissioning 8.6 s/frame and bench packet 19 s
2026-10-22 22:10 UTC fixed both; budget test in novo-app, budgets for every engine step, index equivalence test
2026-10-22 22:00 UTC Windows CI: Ctrl+K step 2.0 s vs 1.5 s on timing alone; DEBUG_FACTOR 12 to 25, packet/report budgets 5 s to 3 s
2026-10-22 22:10 UTC Ctrl+K budget 500 ms to 1 s (3x the Windows 2.0 s figure in debug)
2026-10-22 22:27 UTC scale-2 pass two: wire job cache, async Reports counts, cached page order written; tests running
2026-10-22 22:57 UTC scale-2: review fixes (current-state counts, disconnect, one worker, own-standard cache, test, QA scenario 190 unrun)
2026-10-22 23:00 UTC scale-2: counts forgotten on open/new (R2 repro), scenario renumbered 200, test for two files at one revision
