# WS-13 Reports engine: handoff (rewrite whole file each time)

Updated: 2026-10-14 ~20:30 UTC

## State
- #55 (crate novo-reports) merged as ea47b10 (READY from reviewers B and C on 3a6771c).
- Follow-up PR from main on branch `claude/ws13-reports` (restarted from ea47b10): terminal + cable diagram reports, generated-page sheet naming, frame-aligned PageSetup, C's notes. PR #68; head in PROGRESS.md (code at 74d5d33).
- Review: two-reviewer policy for worker PRs: B (session_01QSnXFBDRyvwwzhT1S1gFGN) and C (session_01Q6QWwh6uhq8Z2C1Ysih3UA). Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47 (merges). WS-28 worker (novo CLI): session_011qgUKo3T6sBPNTVxKy9zZF; it was sent the BOM/xlsx/CSV API.
- GitHub Actions does not run on this account: local fmt/clippy/test are the CI signal.

## Code
crates/novo-reports/src:
- table.rs: Cell, Column, Row, Section, ReportOutput.
- report.rs: Report trait, Dependencies, LiveReports (cache keyed by Stamp {revision, next_id, name}).
- toc.rs, bom.rs, devices.rs, wires.rs.
- diagrams.rs: TerminalDiagram over novo_terminals::screen, CableDiagram over novo_cables::{overview, cable_screens}.
- sheets.rs: generated pages print as sheets named `20` or `20.1`, `20.2`, ...; sheets_of_page, generated_sheets, layout_sheet.
- page.rs: PageSetup defaults sit inside novo-canvas's frame (10 mm) and above its 36 mm title block; paginate, layout_page.
- export.rs: xlsx is deterministic (fixed creation date) and cuts text to 32,767 chars; CSV has excel_safe.
- order.rs: tag_cmp and helpers.

Tests:
- tests/reports.rs: exact contents, paging, CSV, xlsx.
- tests/diagrams.rs: diagrams, sheets, identity guard, cell limit.
- tests/live.rs: cache and 10k timing (debug slack x3).

## Resume
```
git fetch origin && git checkout claude/ws13-reports
export CARGO_TARGET_DIR=~/target-ws13 CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
Merge main with scripts/merge-main.sh. After every push, send the head SHA to B and C. Never push after READY.

## Decisions and traps
- Report pages are PageKind::Generated page records (coordinator decision), whose content is computed live.
- novo-reports depends on novo-terminals and novo-cables. Those crates must never depend on novo-reports (cycle).
- The wire list collapses junctions, interruption points and potential feeds; supply-only wires are omitted.
- Never edit the crate list in CLAUDE.md or the novo-* lines in Cargo.toml (fixed by #51). External dependencies go below them.
