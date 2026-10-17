# DOC-1 Documentation designer: roadmap

Scope: engine for report and form templates, table builder, title block and frame as data, template storage, golden-PDF and pixel-diff tests. Lane DOC-1 in plans/spawn-table.md. Owns `novo-reports/src/designer/` and the `export_layouts_pdf` hook in novo-pdf. UI is a later lane; DOC-2 (languages) builds on these types.

Done (PR 1, this branch `claude/project-thread-bdmgix`):
- Template model (frame, title block, text/line/rect/table elements), serde JSON.
- `{field}` expansion with visible unknown fields; project fields.
- Table builder: columns, headers, widths, filters, natural sort, errors listed.
- Renderer to `PageLayout` with paging and repeated headers; `preview_data` for the live preview.
- Validation (geometry, title block overlap, unknown fields).
- Storage: user folder, project property `designer.templates`, project > user > built-in resolution.
- `novo_pdf::export_layouts_pdf`, golden content stream test, frame pixel-diff tests.
- `designer::ENTRY` (Reports workspace > Designer, command `reports.designer`).
- `report.designer`: opens the report designer, with its language choice (the preview follows the project's report language and refreshes when it changes).

Next, in order:
1. UI lane (R3): Reports workspace > Designer tab: template list, canvas with live preview, table builder panel, title block editor; registers `reports.designer`.
2. Project writes: a command that stores `store::to_property` into the project info (uses `Tx::set_info`; no novo-core change needed).
3. Label printer templates (labels report #482 sizes), barcode/QR element, image/logo element.
4. Cell formatting (number formats, units), grouping and subtotals in tables, multi-section templates.
5. DOC-2: translation keys already carried (`key` on text and title rows).

Open questions: where the designer sits against the existing Reports hub (#452); whether generated pages should use a chosen template per report kind.

## DOC-2 Languages (branch claude/doc-2-languages)
Done in PR 1: `novo_reports::i18n` (Catalog, Translator, ProjectTable, translate_output/template/layout, render_in, missing lists), shipped Dutch file `i18n/nl.json`, project properties `i18n.language` and `i18n.table`, `ReportPdfOptions.language` (PDF, CSV, Excel), File > Report languages (`project.languages`, app/languages.rs).
Done in PR 3: CLI `--language` on export commands and commissioning-report, `novo_py::report_bytes_in`; scripts use the project's language by default (JSON report data stays English). Not translated yet: fixed words used as cell values (commissioning summary labels Checks/Passed/Pending), wire cut job file (machine file). Next: (2) import/export a language file from the window; (3) language on generated schematic pages (`LiveReports::layout_sheet`); (4) locale number and date formats (decimal comma); (5) translate fixed cell words (Yes/No, role names) once they are listed; (6) a vocabulary check that fails when a new `Column::new` header has no Dutch entry.
Decision: Dutch is the report/PDF language only. Menus, command ids and file formats stay English.

PR 4: language files (File > Report languages > Save/Load language file; `Catalog::to_json/from_json`, `*.kopercad-lang.json`). Import is two undo steps (table, then language).

Lane status (2026-10-17): DOC-2 is complete for reports, pages, PDF, CSV, Excel, CLI and Python. Done after PR 4: decimal sign per language (language file field `decimal`, Languages window chips), fixed cell words and assembly checklist texts in Dutch, surface names and rail mount text, numbers in translated patterns follow the decimal sign (measurement placeholders only). Not done, by decision: menus and commands stay English; the wire cut machine file stays English; user-named surfaces and project data are never translated. A new fixed word written by a report must be added to `FIXED_CELL_TEXTS` or `FIXED_CELL_PATTERNS` and `nl.json`; a test fails until it has a Dutch entry.
