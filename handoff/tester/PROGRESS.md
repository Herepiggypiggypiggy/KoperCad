# Tester progress log (append-only)

2026-10-14 12:00 UTC Round 1: tested main and #1-#6; created novo-tests with the Gate 1 fixture; F1 (NaN/inf) and Q1 (overload bridges) reported.
2026-10-14 14:30 UTC Round 2: F1 fixed by #11, Q1 resolved by #12; robustness.rs added; #6 experimental build checked.
2026-10-14 15:00 UTC Round 3: fixture pass_through from #14; 87 tests pass.
2026-10-14 15:26 UTC Round 4: PR #23 (gate0_storage: Gate 1 through .novo, crash recovery, 10k page). Merged.
2026-10-14 16:36 UTC PR #27 merged (all net potentials compared across reload; crash recovery from real ChangeSets; Round 5).
2026-10-14 16:40 UTC Main 11363c1: 232/232 pass; #3 standard-user installer CI passed; Gate 0 summary in PR #32.
2026-10-14 16:50 UTC X1:PE kind fix prepared (tag pe-fix 72b774f), held until #30 merges.
2026-10-14 16:58 UTC PR #32 merged at 5691629 (Gate 0 summary). The e2e draft pushed after the merge (5d4d882) did not reach main.
2026-10-14 17:00 UTC F2 found (terminal tag steals clicks), sent to WS-03.
2026-10-14 17:05 UTC Branch restarted from main 9512822; e2e draft cherry-picked as e14acee. Handoff files written.
2026-10-14 17:20 UTC PR #42 opened (3a98343): Gate 1 scenario through commands passes on main; 7 known-gap tests ignored. Reviewer, lead and WS-01 informed.
2026-10-14 17:08 UTC F2 verified fixed by #28 (924be9b). After #28 merges, make e2e_motor_starter assert the first hit is -X1:2 and mark F2 fixed.
2026-10-14 17:15 UTC WS-01 offered NovoApp::execute hook (their follow-up after #34); noted in ROADMAP.
2026-10-14 17:18 UTC Gate 0 60 fps row marked met (Quadro P1000: 1135 fps avg at 10k). Pushed to #42 as 80dcfe8; reviewer pinged. All Gate 0 criteria met.
2026-10-14 UTC (fixer session_017LndnL4fiT5rRPkqy3R36S): Round 8 on main 7379d42: gap tests for #30 (terminals) and #36 (parts) filled in and un-ignored; pe-fix.patch applied (X1:PE ProtectiveEarth, novo-terminals gate1 check test flipped to clean); F3 fixed in novo-terminals (terminal dropped on the far half of a wire went in backwards). New PR from main on claude/project-thread-dft1m7.
