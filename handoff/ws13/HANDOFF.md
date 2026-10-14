# WS-13 Reports engine: handoff (rewrite whole file each time)

Updated: 2026-10-14 (first session)

## State
- Branch `claude/ws13-reports` from origin/main (9fa18ed). PR #55; reviewed head 5512392 (later commits are handoff-only).
- Review: head SHA sent to the main reviewer (session_01XEdxpvQFh4HWmurh72tKwp) and coordinator (session_01DZ68tTULUv7Uhe8ecKbC47).
- GitHub Actions does not run on this account: local fmt/clippy/test are the CI signal.

## Code
crates/novo-reports/src: table.rs (Cell/Column/Row/Section/ReportOutput), report.rs (Report trait, Dependencies, LiveReports), toc.rs, bom.rs, devices.rs, wires.rs, page.rs (PageSetup, paginate, layout_page), export.rs (xlsx_bytes, save_xlsx, to_csv), order.rs (tag_cmp etc.).
Tests: tests/reports.rs (exact contents on novo-tests motor starter, paging, xlsx/CSV), tests/live.rs (cache + 10k timing).
Also edited: Cargo.toml (`novo-reports`, `rust_xlsxwriter`, `calamine` in [workspace.dependencies]), Cargo.lock, CLAUDE.md crate row.

## Resume
```
git fetch origin && git checkout claude/ws13-reports
export CARGO_TARGET_DIR=~/target-ws13 CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
After each push, send_message the reviewer the new head SHA. No pushes after READY; follow-ups in a new PR from main.

## Decisions and traps
- Uses only novo-core (Part records in the project), not novo-parts.
- Wire list collapses wires at junctions, interruption points (by name), potential feeds and unassigned symbols; connections reaching < 2 device/terminal pins are omitted (supply feeds).
- LiveReports matches output by project revision; one per Document; `clear()` when another project opens.
- Determinism: every report sorts explicitly; no HashMap iteration reaches output.
- Merge conflicts expected on the CLAUDE.md crate table and [workspace.dependencies] with other new crates: keep both rows.
