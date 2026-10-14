# WS-28 progress

2026-10-14 19:00 UTC Started on branch `claude/ws28-python-cli` from main 9fa18ed.
2026-10-14 19:40 UTC novo-py ScriptProject + PyO3 shim (feature `python`), novo-cli; wheel builds with maturin 1.15 / pyo3 0.29 (abi3-py39) and installs with `pip install --user`.
2026-10-14 19:55 UTC Local checks green: fmt, clippy -D warnings, cargo test --workspace, pytest (3 passed, BOM skipped for WS-13).
2026-10-14 20:05 UTC Pushed e828573, opened PR #56; handoff commit follows, reviewer and coordinator pinged with its SHA.
2026-10-14 19:20 UTC CI on 9bb08e4 failed with no runner assigned (same on main run 214); PR comment posted. Clean trial merge with main 7379d42, crates green. Routed to reviewer B (session_01QSnXFBDRyvwwzhT1S1gFGN). Handoff commit held locally to avoid moving the head; push with the next change.
