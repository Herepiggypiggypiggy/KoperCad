# WS-28 progress

2026-10-14 19:00 UTC Started on branch `claude/ws28-python-cli` from main 9fa18ed.
2026-10-14 19:40 UTC novo-py ScriptProject + PyO3 shim (feature `python`), novo-cli; wheel builds with maturin 1.15 / pyo3 0.29 (abi3-py39) and installs with `pip install --user`.
2026-10-14 19:55 UTC Local checks green: fmt, clippy -D warnings, cargo test --workspace, pytest (3 passed, BOM skipped for WS-13).
2026-10-14 20:05 UTC Pushed e828573, opened PR #56; handoff commit follows, reviewer and coordinator pinged with its SHA.
2026-10-14 19:20 UTC CI on 9bb08e4 failed with no runner assigned (same on main run 214); PR comment posted. Clean trial merge with main 7379d42, crates green. Routed to reviewer B (session_01QSnXFBDRyvwwzhT1S1gFGN). Handoff commit held locally to avoid moving the head; push with the next change.
2026-10-14 19:40 UTC Reviewer B READY on 19c1f00 (6 non-blocking notes); reviewer C (session_01Q6QWwh6uhq8Z2C1Ysih3UA) CHANGES NEEDED on the same head: create/save_as replaced a crashed file and deleted its recovery file. Reproduced, fixed (save_as refuses RecoveryPending unless it is the session's own file), Rust + pytest tests added; pushed to #56 since merging 19c1f00 would ship the data loss. B's 6 notes still go to a follow-up PR.
2026-10-14 19:45 UTC B withdrew READY (CHANGES NEEDED on 0089b7c, same blocker as C). Coordinator's main merge 0089b7c (post-#51 crate list) is in. Added overwrite=False guard on create/save_as (B's note 1; recovery refused even with overwrite). Remaining follow-ups: plain clap errors, novo_checks (#50 merged), novo-pdf (#49), no panic in execute_one, atomic export write, venv-first README.
2026-10-14 19:50 UTC C READY on ac037dc. Merged main (#49 pdf-writer) into e5bb8ae: Cargo.toml keep-both, main's Cargo.lock + cargo metadata; checks green.
2026-10-14 19:50 UTC #56 READY from B and C on e5bb8ae; merged as 67c3591. Branch restarted from main for the review follow-ups (pdf on novo-pdf, check on novo-checks, export guard + atomic write, plain clap errors, NothingCreated, venv README).
2026-10-14 20:05 UTC Opened PR #65 (follow-ups); head is the commit adding this line. Pinged B, C, coordinator.
2026-10-14 20:20 UTC #55 merged; added reports to #65 (novo export bom/devices/wires/toc/reports, Python add_part/assign_part/report/export_bom/export_reports). test_dol_bom_to_excel un-skipped and passing: done-criterion met headless. Main (ea47b10) merged in.
