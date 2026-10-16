# pdf-export: reviewer notes

Teaches: issue a revision, export the set as a vector PDF, and what to check in the file (bookmarks, links, title block, page labels), plus what an
issue freezes and what a plain export does not.

Project: start = the finished state of wire-list-and-bom (the same drawing, lamp with its part). Finished = revision A issued (`Tx::issue_revision`):
drawn pages frozen with "A", generated sheets not. documents.rs exports the finished project with novo-pdf and asserts bookmark texts, links and
title block text.

Verification: `crates/novo-app/src/app/lesson_verify_outputs.rs` (`pdf_export_every_step_works_from_its_own_start`, `..._solve_scripts_replay`,
`the_menu_routes_and_the_page_list_click_work`).

Verified path per step
- issue-open: {key:view.command_palette} + "issue" or File > Revisions > Issue revision (commands.rs revision.issue, menus.rs); the window "Issue revision A"
  has the fields **What changed**, **Index**, **Date**, **By** (revisions.rs); both routes driven.
- issue-submit: typed text + click **Issue A** driven; asserted: revision A "First issue", message "Issued revision A", drawn pages frozen, generated not.
- title-block: novo-canvas model.rs (140 x 36 mm, bottom right).
- export: palette "export pdf" or File > Export > Export PDF (both driven); the file `Pump.pdf` is written, begins %PDF, has Outlines.
- where-is-it: pdf.rs (a lesson project is not saved: Documents\KoperCAD\<name>.pdf; the test saves first).
- check-bookmarks / check-links: novo-pdf lib.rs; the Design checks appendix when problems remain (pdf.rs checks_appendix).
- freeze: tx.rs issue_revision; reports.rs add_generated_pages adds nothing to a project with frozen pages (and says so).

GAPS
- Choosing pages: the app always exports every page (pdf.rs). Only `novo_pdf::PdfOptions::pages` and `kopercad-cli export pdf --page` can choose.
  The last step says so.
- No check kind for "a revision was issued"; a reading step cannot carry a solve, so issue-submit is a reading step (the test does the action).
  Wished: check `revision = { index = "A" }` (then issue-submit gets expect + solve `issue_revision`, which exists).
- drawn_by / checked_by have no control (project_settings.rs); the sample project sets them.
- The PDF is read outside the app, so the checking steps are reading steps; nothing in the app opens the file.
- Anchors a pointer would use do not exist (`revision.submit`, `mark.menu.export`, `palette.field`).
- Opening a project with frozen pages adds no generated pages (confirmed in reports.rs); the freeze step says so.

## Pending how

- intro: (reading)
- issue-open
  - en: ["Press {key:view.command_palette}, type **issue**, press Enter", "Or click **File**, **Revisions**, **Issue revision**"]
  - nl: ["Druk op {key:view.command_palette}, typ **issue**, druk op Enter", "Of klik op **File**, **Revisions**, **Issue revision**"]
- issue-submit
  - en: ["Type **First issue** in **What changed**", "Check **Index**, **Date** and **By**", "Click **Issue A** (or press Enter)"]
  - nl: ["Typ **First issue** bij **What changed**", "Controleer **Index**, **Date** en **By**", "Klik op **Issue A** (of druk op Enter)"]
- title-block: (reading)
- export
  - en: ["Press {key:view.command_palette}, type **export pdf**, press Enter", "Or click **File**, **Export**, **Export PDF**"]
  - nl: ["Druk op {key:view.command_palette}, typ **export pdf**, druk op Enter", "Of klik op **File**, **Export**, **Export PDF**"]
- where-is-it, check-bookmarks, check-links, freeze, pages-and-next-time: (reading)
