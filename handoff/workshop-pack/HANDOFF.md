# Workshop pack: handoff

Branch `claude/project-thread-o9kk2b` (kopercad). Code:
- `crates/novo-layout/src/template.rs`: `Paper`, `TilePlan`/`plan_tiles` (fewest sheets, portrait or landscape, 10 mm overlap), `TemplateSheet`, `template_sheets`; `model::template_documents(project, id)`.
- `crates/novo-pdf/src/template.rs`: `export_template_pdf(sheets, project, paper)`; overview page then tiles row by row. `PageWriter::begin_clip/end_clip` added to clip tiles.
- `crates/novo-app/src/app/workshop.rs`: `export_workshop` writes the folder; Bench output window strip and command `export.workshop` (Reports > Export).
- Tests: `cargo test -p novo-layout --lib template`, `cargo test -p novo-pdf --test template`, `cargo test -p novo-app workshop`.
- To look at a sample: `TEMPLATE_DUMP=/tmp/t.pdf cargo test -p novo-pdf --test template -- --ignored dump`.

Decisions: true size depends on the print dialog, so every sheet says print at 100% and carries a 100 mm ruler (a test checks it is 100.00 mm in the PDF). Accuracy on paper (about 0.5 mm) is an estimate, never measured here; the overview says so. Hole positions come from the drill table, so devices without hole data show an outline only and are listed. Not printed on a real printer in this session.
Traps: window and command share `wb.workshop_paper`; existing Bench exports (`export_*_to`) are reused so the pack always agrees with them.
