# Single-line diagram: handoff

Branch `claude/project-thread-1ndgb2`. Open PR: see PROGRESS.md for the number and head SHA.

State: first PR complete and tested; waiting for review. Do not push once READY; follow-ups go in a new PR from main.

Where things are:
- Engine: `crates/novo-checks/src/single_line.rs` (tests `crates/novo-checks/tests/single_line.rs`). Reuses `standards/netclass.rs` (`Nets::distances_with` added), calc::drop, calc::cable (`rating_with`), calc::motor, Autopilot `plan_for`.
- Report and drawing: `crates/novo-reports/src/single_line.rs`; hook in `sheets.rs::layout_sheet`.
- App view: `crates/novo-app/src/app/single_line_view.rs` (+ `_tests.rs`), command `report.singleline` in novo-workflow.
- Fixture: `crates/novo-tests/src/distribution.rs`.

Resume: `cargo test -p novo-checks --test single_line -p novo-reports --test single_line -p novo-pdf --test single_line_pdf -p novo-app single_line_view`. QA: `scripts/qa.sh --only single-line` (needs xvfb openbox mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0).

Honesty: marks on the drawing: none = stated, `~` calculated, `?` assumed, `*` Autopilot proposal. Iz and voltage drop are always calculated. Ampacity tables are planning values. IEC 60204-1 clause numbers unverified. No diversity. Supply taken at the first device. Disk: `scripts/free-disk.sh`; use CARGO_INCREMENTAL=0.

Traps: a transformer or power supply carries power, not summed amps, and restarts the ΔU sum; the Reports menu holds at most 15 plain rows (plug list and single-line diagram sit in "More reports"); adding a report also needs the Dutch strings, the cli sheet list and the live-report stale lists. Also: a fixture run protected by an upstream breaker is judged against that breaker (In > Iz); the fixture uses 16 mm² for that reason.
