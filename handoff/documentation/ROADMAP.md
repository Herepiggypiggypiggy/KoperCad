# Documentation designer and languages (gap 11, docs/AMBITION-REVIEW.md brief 11)

Status 2026-10-15: planned only, no code written. Paused on Tadjiro's request (usage limit).

## What
1. Title-block and frame designer: templates of cells (caption, value from a field such as project, page, sheet, structure, revision, drawn/checked, logo, free text), sizes in mm, live preview.
2. Report-layout designer: table builder (columns, widths, order, header text, row height) per report, saved as project templates.
3. Project-level language: one project language plus extra output languages, a translation table for custom texts, language switch on PDF and Excel output.
4. Complete Dutch: UI strings, generated report headers, title block captions, terminal and cable sentences.

## How (start points found in the code)
- Title block is hard-coded in `crates/novo-canvas/src/model.rs` `title_block()` (six English captions, fixed width). Replace with a template rendered from a `TitleBlockTemplate` read from the project.
- Report headers are English literals in each `crates/novo-reports/src/*.rs` (`Column::new(key, header, width)`). Keep `key` stable, translate `header` through a catalog by key.
- `crates/novo-terminals/src/strings.rs` and `crates/novo-cables/src/strings.rs` already collect sentences in one place: first to translate.
- `crates/novo-workflow/src/language.rs` already holds Dutch command aliases; the UI labels have no catalog yet.
- Storage without a novo-core API change: project info properties (`language`, `languages`, templates and translation table as JSON text). A novo-core change, if wanted, goes to WS-02.
- Engine in `novo-reports` (catalog, templates, field resolver), pure and tested; a crate split needs a coordinator PR first.
- Tests: golden PDFs per template, "switching language changes every string" test over all reports, pixel-diff or primitive-diff test on frames, catalog completeness test (every English key has a Dutch entry).

## Where the user finds it
Reports workspace > Designer (title block, report layouts), Project > Languages (project language, output languages, translation table), language choice in the PDF export dialog.

## Order
1. Catalog and Dutch for reports, terminals, cables (testable, no UI).
2. Title block template engine, replacing `title_block()`.
3. Languages window and PDF/Excel language switch.
4. Designer UI with live preview.
5. Dutch UI labels in novo-app.
