# Print preview and plot sets: handoff

Branch: claude/project-thread-xx2ycy (kopercad). One PR from main; no stacked PRs. Follow CLAUDE.md.

## State
Engine and UI are in the PR (see ROADMAP.md "Done"). Tests: `cargo test -p novo-pdf` (print unit tests, tests/print_setup.rs), `cargo test -p novo-app --lib print_preview`.

## Where things are
- crates/novo-pdf/src/print.rs: setup types, `place()`, `footer_text()`.
- crates/novo-pdf/src/lib.rs: `draw_sheets()` (DrawnSheet/DrawOp), `footer_labels()`, `export_pdf_with_reports()` writes from them. Without `PdfOptions.print` the output is byte-identical to before (golden tests unchanged).
- crates/novo-pdf/src/draw.rs: `begin_placed/end_placed` (scale, offset, clip), minimum pen 0.15 mm on paper.
- crates/novo-app/src/app/print_preview.rs: window, painter, plot sets, workshop tab. Settings: `print_choice`, `plot_sets` in settings.rs.
- crates/novo-app/src/app/pdf.rs: `pdf_options_for()` and `export_options_to()` split out of the old export so preview and one-click export share them.

## Traps
- Plot sets store page names, not ids, keyed by `pdf::owner_tag(project file)`; an untitled project uses the key "untitled".
- The preview redraws sheets when the ticked pages, look or `project().revision()` change (not per frame).
- A fixed scale that does not fit clips at the printable box and warns; Fit never clips.
- Page-number strip is 9 mm minimum at the foot of the paper.
