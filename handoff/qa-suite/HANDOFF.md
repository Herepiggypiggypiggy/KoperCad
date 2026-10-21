# QA suite handoff

Branch: claude/project-thread-03q1dh (kopercad). One PR: "Test and screenshot suite".

State: #1074 merged. Follow-up PR has the harness fixes. Full command sweep (270 commands, 33 lessons) ran headless: no crashes.
Quick scenarios (01 to 06) pass or give real findings; sweeps (90, 91) take about 3 s per command
(about 14 min for all commands at one size).

Resume
- `cargo build -p novo-app -p novo-cli`, then `scripts/qa.sh --size 1366x768 --out /tmp/qa`.
- Needs `apt install xvfb openbox mesa-vulkan-drivers libxkbcommon-x11-0` on Linux.
- Add a scenario: copy a file in `qa/scenarios/`, use `tree` to find labels (docs/testing/qa-suite.md).

Traps
- The same label can be a menu and a workspace button ("Reports"): use `#2`.
- A window manager may ignore the first resize; `ensure()` in testuser.rs now retries until the size is right.
- The golden .novo has no panel: Cabinet is empty and 3D is disabled.
- layout check is a heuristic (see docs); never claim a clean screen from it alone.
- Do not rebuild kopercad-cli while a long sweep runs from target/debug: copy the binaries first.

Talk to: coordinator for roster; Reviewer A for READY; merge lead by PR parity.

## Timing-robust tests (CI hygiene, 2026-10-21)

Converted so a slow debug runner cannot flake them; no behaviour change, none skipped:
- `novo-tests/tests/gate0_stress.rs`: page read 500 ms, edit and undo 200 ms budgets now `budget()`, ten times longer in debug builds, unchanged in release (`scripts/scale-bench.sh`).
- `novo-cfd/tests/sched.rs` cancel deadline 2 s to 10 s; `novo-cfd/tests/cabinet.rs` cancel deadline 5 s to 20 s (both already poll with a deadline).
Already robust (debug slack or deadline polling, left as is): `novo-canvas/tests/perf.rs` (20x debug slack), `novo-reports/tests/live.rs` (slack), heat tests (#1192), `parts_import_tests`, `heat_time_tests`, `heat_run_tests`, `novo-cfd/tests/sched.rs` other loops. `novo-parts/tests/fit.rs` 2 s is already a loose debug bound.
