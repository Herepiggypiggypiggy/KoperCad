2026-10-21 13:05 UTC Thread started from the scale handoff.
2026-10-21 13:30 UTC Netlist cache per revision (Arc), test in novo-core/tests/potentials.rs.
2026-10-21 13:40 UTC Design check background job (checks.rs), tests in checks_background_tests.rs.
2026-10-21 13:50 UTC Operate list / buttons-elsewhere cache in live.rs.
2026-10-21 14:00 UTC frame_stats in control mode + qa step; options bench step; options::evaluate without record clones (33 -> 17 ms, copy 73 -> 53 ms at 600 pages).
2026-10-21 14:30 UTC Sampled the window (gdb stacks): 14 of 14 samples in suggest_terminals via page_proposals. Added Project::derived, cached proposals: still frame 105 ms to 7 ms in the test harness.
2026-10-21 15:10 UTC QA scenario 40-scale-frames before/after at both sizes (reports in /mnt/project-files/qa/responsiveness-scale600*).
