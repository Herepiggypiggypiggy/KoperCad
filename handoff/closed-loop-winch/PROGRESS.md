2026-10-29 11:15 UTC Thread started from Tadjiro's goal: simulate niche and complex situations (closed-loop hoists, synchronous winches).
2026-10-29 12:00 UTC Closed-loop engine, winch unit, findings and 25 hand-calculated tests in novo-drive.
2026-10-29 12:30 UTC Two-winch pair (rope stretch, tilt, sync modes, partner stop) and 12 tests in novo-machine; six scripted scenarios with 10 tests.
2026-10-29 12:50 UTC Window Simulate > Winch scenarios (sim.winches).
2026-10-29 16:30 UTC PR #1712 and the UI nits #1725 merged. Schematic binding: `cw_*` section, `cl_winch_spec`, `cl_winch_loop`, Drive run rows and findings, fixture `closed-winch.novo`, tests with injected coil defects.
2026-10-29 18:00 UTC #1736 merged (5963bcb2). Follow-up #1741 (findings cap, thresholds text). Pair on the schematic: `cl_pair_loop`, `PAIR_KEYS`/`closed_pair_from_properties`, `WinchLine` on `PairCommand`, Drive run rows and badges per winch, section "Two winches on one beam", fixture `closed-winch-pair.novo`, scenarios 1738 to 1738d.

- 2026-10-29: #1736 merged. Follow-up PR: the closed-loop winch keeps at most `MAX_FINDINGS` (200) findings and counts the rest ("N more findings not shown" in Drive run); the honesty text lists the 5, 20 and 50 mm report thresholds. Next: two winches on one beam from the schematic, walk-throughs, drawn single-channel encoder loss.
2026-10-29 21:50 UTC #1741 and #1751 merged. Walk-throughs run the winch and pair loops (`winch_check`, `Machinery`, `run_with_machinery`), `Check::Winch`, app wiring in `defects_card::run_walkthrough`, fixture `closed-winch-walk.novo`, scenario 1752.
