# rel-j-small-fixes: handoff

Branch `claude/project-thread-brfexy` (kopercad), one DRAFT PR, kept draft until the coordinator says v0.1.0-rc1 is tagged.

Changes:
- `novo-canvas/src/widget.rs` `fit_scene`: Fit::Content shows the whole sheet when the drawing would be less than 1.5x closer than the sheet fit (`FIT_SHEET_RATIO`). Test `a_drawing_that_nearly_fills_the_sheet_fits_the_whole_sheet`.
- `novo-workflow/src/commands.rs`: `export.pdf_options` title is "Export PDF with options...".
- `novo-app/src/app/pdf.rs`: `pdf_base()` gives the PDF folder (always `projects_dir()`), used by `pdf_path`, `export_pdf_page` and the options dialog. Test hook `wb.pdf_dir` (cfg(test)) points tests at a temp folder.
- Lesson `pdf-export` text (en, nl) says the PDF is in Documents\KoperCAD.

Where the user finds it: opening the example (Start > Open example) at a wide window; File > Export drawings; the toast after an export names the folder.

Verified on Linux (Xvfb, `--control`, 1920x1080): before and after screenshots. Windows not run.
