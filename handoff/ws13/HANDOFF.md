# WS-13 Reports engine: handoff (rewrite whole file each time)

Updated: 2026-10-14 ~20:25 UTC, cloud session wrap-up (session_01834PmEwNeusq2TnHgA5ZyG).
Crate: `crates/novo-reports` (owner WS-13). Engine only, no UI.

## State
- **#55 merged** as ea47b10. It added the crate: framework, toc/bom/devices/wires, paging, xlsx/CSV.
- **#68 merged** (2026-10-14 ~20:30 UTC, squash 8e13bc0, final head ce960a5): "WS-13: terminal and cable diagram reports, generated-page sheets, review follow-ups". The coordinator added a main merge and a one-line novo-cli test update (the reports workbook now has the "Terminal diagram" and "Cable diagram" sheets); B confirmed it on ce960a5.
  - Branch `claude/ws13-reports`, head **02d6e29** (code at 74d5d33).
  - Reviewer B (session_01QSnXFBDRyvwwzhT1S1gFGN): **READY at 02d6e29**, all green when merged with main cdf2402.
  - Reviewer C (session_01Q6QWwh6uhq8Z2C1Ysih3UA): **READY at 02d6e29** too. Both verdicts were in; it is merged, so don't reuse the branch.
  - It also carries handoff/ws13 edits. This wrap-up PR (claude/handoff-ws13) supersedes them; on conflict, keep this version.
- **#72** (fixer session_017LndnL4fiT5rRPkqy3R36S, reviewer A), branch claude/fix-terminal-part-one-source, head 38c3070.
  - Moves `novo_terminals::terminal_part(project, terminal) -> Option<PartId>`, which resolves Terminal.part, else the style override, else the strip's `terminal_part.<kind>`. It does not include strip device parts.
  - Patches `novo-reports/src/bom.rs` (terminal uses loop and the without_parts coverage check) and adds `novo-terminals.workspace = true` to novo-reports/Cargo.toml. #68 adds the same line: whichever merges second gets a trivial conflict, keep one line.
  - **Stay off bom.rs until #72 merges.**
- GitHub Actions does not run on this account. Local fmt, clippy and test are the CI signal.

## What exists (after #68)
- `table.rs`: Cell (Text/Int/Number/Empty), Column, Row (with model `sources`), Section, ReportOutput.
- `report.rs`: the `Report` trait and Dependencies. LiveReports keys its cache by a Stamp (revision + next_id + project name). `notify` / `notify_since` take ChangeSets; `get` / `all` regenerate lazily; `clear()` on project open.
- Reports (ids): toc, bom, devices, wires, terminals (TerminalDiagram over `novo_terminals::screen`), cables (CableDiagram over `novo_cables::{overview, cable_screens}`).
- `page.rs`: PageSetup defaults sit inside the novo-canvas frame (10 mm) and above its title block (140×36 mm). `paginate` repeats the header row; a report with no sections still gets one page. `layout_page` returns TextBox and Line in mm (top-left origin, y down).
- `sheets.rs`: generated pages print as sheets (proposal below). `sheets_of_page`, `generated_sheets`, `layout_sheet`.
- `export.rs`:
  - xlsx: deterministic (fixed creation date), text cut to 32,767 chars, sheet names sanitised, `save_xlsx` writes a temp file then renames.
  - CSV: `excel_safe` prefixes `'` to text starting with = + - @ tab CR; `;` gives decimal commas; optional BOM.
- Tests: tests/reports.rs, tests/diagrams.rs (#68), tests/live.rs. The 10k timing test runs ~145 ms in debug; the bound is 1 s, ×3 slack in debug.

## Multi-sheet proposal (in #68, not yet adopted by the renderers)
- A `PageKind::Generated { generator }` page record prints as **its own name** when its report fits one sheet, otherwise as **`<name>.1`, `<name>.2`, …** (e.g. 20.1, 20.2).
- Dotted page names are the model's existing sub-page convention and sort naturally between 20 and 21.
- API:
  - `LiveReports::sheets_of_page(project, page)` returns `Vec<GeneratedSheet { page, report, name, sheet }>`.
  - `generated_sheets(project)` returns all of them in page order.
  - `layout_sheet(project, &sheet)` returns a `PageLayout` using `PageSetup::for_format(page.format)`.
  - An unregistered generator gives no sheets: an empty frame, never an error.
- **WS-03 (canvas) needs to**: for a Generated page, draw `layout_sheet` text boxes and lines inside its frame, offer sheet navigation, and show `GeneratedSheet::name` plus "n of m" in the title block.
- **WS-09 (novo-pdf) needs to**: emit one PDF page per GeneratedSheet with page label/bookmark = sheet name, draw TextBox/Line (clip text to box width), and keep the frame and title block.
- **WS-01 (app)** needs to keep one LiveReports per Document, call notify for every transaction/undo/redo, and call clear() on open.

## Open notes, not yet done (do them in a new PR from main after #68 and #72 merge)
1. (B and C, #68) Sheet names like `20.1` can collide with a real page named `20.1`. Pick a rule (e.g. detect the collision and fall back to `20/1`, or forbid real pages named like generated sheets), document it and test it.
2. (B, #68) CableDiagram builds every cable screen twice (`overview()` calls `cable_screens()` internally). Build the screens once and derive the overview rows locally, or ask WS-12 for an overview-from-screens function.
2b. (C, #68) Pinned flow vs growing generated pages: DESIGN wants page.column references frozen after a revision or export, but a generated page that grows from 2 to 3 sheets renumbers 20.2 and later sheets. Decide with the workflow thread, e.g. freeze a report's sheet split at revision time and append new sheets at the end.
3. (Coordinator, from reviewer A) After #72: add a novo-reports BOM test with a strip-level terminal article (`terminal_part.<kind>` on the strip). Expected: one BOM line counted per terminal, Used by `-X1:1, …`.
4. Command-field verbs (`bom`, `export xlsx`): owned by WS-08/WS-01. WS-28 (#65) already exports via the CLI and Python, and plans a generic `novo export report <id>`.
5. Report templates (user column choice, order, headers), joined-terminal accessories (JoinLink) in the BOM, wire-list sort by wire number, frozen page.column after a revision.

## Resume
```
git fetch origin && git checkout claude/ws13-reports    # only if C asks for changes on #68
# otherwise, after #68 (and #72) merge:
git checkout -B claude/ws13-reports origin/main
export CARGO_TARGET_DIR=~/target-ws13 CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
Open PRs yourself; the body starts with `_Requested by **Tadjiro**._`. Send head SHAs to B and C after every push. The coordinator (session_01DZ68tTULUv7Uhe8ecKbC47) merges.

## Traps and decisions
- Report pages ARE pages: Generated page records whose content is live (coordinator decision, DESIGN §2).
- novo-reports depends on novo-terminals and novo-cables, never the reverse.
- The crate list and `novo-*` lines in Cargo.toml and CLAUDE.md are fixed: never edit them. External deps go below them. `scripts/merge-main.sh` aborts when main adds external deps next to ours; resolve by hand (keep both).
- Report output must stay deterministic: explicit sorts, no HashMap order, fixed xlsx creation date.
- The wire list collapses junctions, interruption points and potential feeds; connections reaching < 2 device pins are omitted.
