# Revision diff on the canvas: handoff

Branch for new work: start from main (all earlier PRs merged or READY). No work half-done.

## State (2026-10-15 19:50 UTC)
- #401, #442 merged.
- #469 PDF change report: READY (code reviewer B, design reviewer B) on 0c1ba092da2c600b4968eb47b2aa233668e64275. Do not push to it; the integration lead merges.
- #485 legend plus next/previous change (base was #442's branch, retargets to main): in review with reviewer C, head 20b57fbda8b5aa41bfb58cbbccee778cde624fdc.

## How to reach it (UI entry points)
- Outline > Changes: change rows per page, click selects and reveals the object (`changes` command, needs an issued revision).
- Changes card on the canvas: legend, Previous / Next (#485), Compare… button (opens the Revision tracking thread's Compare window).
- Palette: `next change`, `previous change` (#485); `export change report` (#469, writes `<project> changes <index>.pdf` next to the project; Export menu item is the reports UI thread's, #452).

## Where the code is
- `crates/novo-app/src/app/changes.rs`: Changes mode, rows cache, `reveal_change`, `step_change`, legend.
- `crates/novo-app/src/app/change_report.rs`: the export command; `crates/novo-revisions/src/report.rs`: `change_report(&Comparison, title, page_name) -> ReportOutput`; printed by `novo_pdf::export_report_pdf`.
- Engine and `ChangeRow`: `crates/novo-revisions/src/{list,compare,fields}.rs` (Revision tracking thread owns these).

## Traps
- Main moves fast: merge main (never rebase a pushed branch) and expect conflicts in `app.rs` mod list, `workbench.rs` command lists and `changes_tests.rs`; keep both sides.
- PR review routing is by PR number mod 3 (0 main reviewer, 1 reviewer B, 2 reviewer C); design verdict from the design reviewer; never push after READY.
- One `novo-app` test (`parts_import ... the_watcher_picks_up_a_file_named_after_the_article`) is flaky under load, not ours.

## Who to talk to
Revision tracking and compare: session_01SRPgw1A9Q7haevj9AyydQD. Integration lead: session_01FiLvRBKEcNNHbQEGhYyL1S. Reports UI: session_01TaTTPuTqhk8bHd277oJRFR. Menus: session_01WAQSEGkMbyreCPG18tsSPc.
