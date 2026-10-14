# WS-13 Reports engine: roadmap

Roadmap scope: generated-page framework, bill of materials, device list, wire
list, table of contents, report templates, Excel export. Done when reports
regenerate within a second of any edit.

Crate: `crates/novo-reports` (owner WS-13). Engine only; no UI.

## Done (first PR, branch `claude/ws13-reports`)
- Typed table model: ReportOutput → Section → Row → Cell (Text/Int/Number/Empty), Column (header, align, relative width), row `sources` (model ids, for jump-to later).
- `Report` trait (pure fn Project → ReportOutput, `dependencies()` by EntityKind + info) = the seam for WS-11 terminal diagrams and WS-12 cable diagrams. Documented in lib.rs.
- `LiveReports`: notify(ChangeSet) invalidates only affected reports, `get` regenerates lazily; a missed notification can't leave a stale report (rolls forward only from revision-1).
- Reports: toc, bom (grouped by article; device, terminal and cable parts; cable lengths; "devices without parts" section), devices (=function, +location, parts, page.column locations), wires (pin-to-pin connections; junctions, interruption points, potential feeds collapsed; T-junctions as daisy chains; potential via netlist; cross-section from wire or cable core; cable core).
- Paging (`paginate`: header repeated, each section on a new page) and `layout_page` → text boxes + rules in mm for WS-03/WS-09 to draw.
- Excel (rust_xlsxwriter, one sheet per report, numbers as numbers) and CSV (RFC 4180, `;` + decimal comma option, BOM).
- Tests: exact contents on the Gate 1 fixture, paging, determinism, xlsx round trip (calamine), CSV, timing (10k objects: ~170 ms debug for a worst-case edit + all reports paged and laid out).

## Next, in order
1. Command-field verbs `export xlsx` / `export csv` / `bom` etc. in novo-workflow (WS-08) or novo-app (WS-01), reading `LiveReports`. Ask those threads; keep this crate engine-only.
2. App wiring (WS-01): one `LiveReports` per Document, notify on every transaction/undo/redo, `clear()` on project open.
3. Canvas/PDF drawing of `PageLayout` (WS-03 / WS-09 #49): agree PageSetup margins/title block with their frame.
4. Report templates: user-chosen columns/order/headers per report (a `Template` that filters and renames `Column.key`s), stored in project info properties or a profile file.
5. WS-11 adapter: `TerminalDiagram: Report` in novo-terminals once #30 merges (one Section per strip from `screen()`).
6. BOM: joined terminals' connectors/jumpers (JoinLink) as accessory lines; quantities per structure (=A1 / +CAB) as an option.
7. Wire list: sort by wire number option; frozen page.column after revision (DESIGN "pinned flow").

## Open questions
- Should generated report pages appear in the model as `PageKind::Generated` page records (so they get page names and appear in the TOC), or stay command-only views (DESIGN says "reports are commands")? Current code supports both: `LiveReports::report_of_page`.
