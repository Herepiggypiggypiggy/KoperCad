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

Next, in order:
1. UI lane (R3): Reports workspace > Designer tab: template list, canvas with live preview, table builder panel, title block editor; registers `reports.designer`.
2. Project writes: a command that stores `store::to_property` into the project info (uses `Tx::set_info`; no novo-core change needed).
3. Label printer templates (labels report #482 sizes), barcode/QR element, image/logo element.
4. Cell formatting (number formats, units), grouping and subtotals in tables, multi-section templates.
5. DOC-2: translation keys already carried (`key` on text and title rows).

Open questions: where the designer sits against the existing Reports hub (#452); whether generated pages should use a chosen template per report kind.

## DOC-2 Languages (branch claude/doc-2-languages)
Done in PR 1: `novo_reports::i18n` (Catalog, Translator, ProjectTable, translate_output/template/layout, render_in, missing lists), shipped Dutch file `i18n/nl.json`, project properties `i18n.language` and `i18n.table`, `ReportPdfOptions.language` (PDF, CSV, Excel), File > Report languages (`project.languages`, app/languages.rs).
Next: (1) CLI `--language` and Python `report_bytes(language=)`; (2) import/export a language file from the window; (3) language on generated schematic pages (`LiveReports::layout_sheet`); (4) locale number and date formats (decimal comma); (5) translate fixed cell words (Yes/No, role names) once they are listed; (6) a vocabulary check that fails when a new `Column::new` header has no Dutch entry.
Decision: Dutch is the report/PDF language only. Menus, command ids and file formats stay English.
