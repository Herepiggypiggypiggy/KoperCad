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
2026-10-14 UTC Round 9 on main 15c9992: scenario_through_the_window (Gate 1 through NovoApp::execute, #53) added; #38 gap tests (place by search with auto tags and self-wiring; LibraryArt drawing) filled in and un-ignored. BOM terminal count check left to #72. Workspace: 628 passed, 0 failed, 1 ignored. PR #87 opened at a28066b; reviewer A pinged.
2026-10-14 21:55 UTC Round 10: main 7bb466d re-tested locally (625/0/3). Main merged into #87; outputs_of_the_starter added (checks clean, xrefs, BOM, PDF). F4 (BOM orders 1 terminal block for 4 terminals when the catalog part sits on the strip) and F5 (printed page never names -X1) found; ignored gap tests added. Workspace 629/0/3.
2026-10-14 21:50 UTC #87 at 0434c4f (mcb3 -F1 pins current behaviour; O1 open). Reviewer A stopped; sent to main reviewer cse_016C9nTT8cN7voW8t1NQUG4W. CI red only from Actions billing outage (commented on PR). #72 closed; WS-07 redoing it, re-test when they ping.
2026-10-14 22:00 UTC F4 gap test tightened (per-terminal order numbers; PE gets 3044089 or is reported missing). Lead decided WS-07's strip-style fix is the one answer; Outputs drops its strip-device fallback.
2026-10-14 22:05 UTC main 067e56a merged into #87: fmt/clippy clean, 646/0/3.
2026-10-14 22:15 UTC #99 (854f8cf) re-tested on main 067e56a + #87: 647/0/4. F4 split: (a) missing parts lists the strip device -> fixed by #99; (b) assign_part still puts a terminal block on the strip device -> open. outputs test now assigns terminal articles (3 x UT 2,5 + UT 2,5-PE) and checks the BOM. #99's own gap test gives PE the grey block.
2026-10-14 22:20 UTC #99 merged; main c3c51a0 merged into #87, F4 tests un-ignored, textures_delta comment added. 659/0/2. F4 closed.
2026-10-15 04:15 UTC Round 11 (F5 closed by #126) and Round 12 (Gate 2 assessment, gate2_live_pages.rs: engine live; gaps G2-1..G2-4) on main f3534dc: 722/0/3.
2026-10-15 03:50 UTC #146 merged. Follow-up PR: reviewer nits (BOM columns by key, -W1 table by tag).
