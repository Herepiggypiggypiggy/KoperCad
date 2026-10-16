# pdf-export: reviewer notes

Teaches: issue a revision, export the whole set as a vector PDF, and what to check in the file (bookmarks, links, title block, page
labels), plus what an issue freezes and what a plain export does not.

Project: start = the finished state of wire-list-and-bom (honest: it is the same drawing, only the lamp's part differs from lesson 1).
Finished = revision A issued (`Tx::issue_revision`): drawn pages frozen with "A", generated sheets not. documents.rs also exports the
finished project with novo-pdf and asserts the bookmark texts, the links between the contactor and its coil and the title block text.

Verified UI path per step
- issue-open: Ctrl+K keymap.rs:313; verb "issue" commands.rs:215-223; window revisions.rs:71-84.
- issue-submit: dialog revisions.rs:86-149 (button "Issue A", Enter issues too; initials may be empty).
- title-block: novo-canvas model.rs:1197-1285 (140 x 36 mm, bottom right of the inner frame = x 270, y 251 on A3).
- export: no key; palette verb `export pdf` commands.rs:714-721; KoperCAD menu button "Export PDF" app.rs:959.
- where-is-it: pdf.rs:1-78 (next to the project file, own PDFs never replaced, "(N remain)" never blocks).
- check-bookmarks / check-links: novo-pdf lib.rs:522-543, 556-612 (links go from a device symbol and its tag to its partner on another page).
- freeze: tx.rs:402-433; opening a project with frozen pages adds no generated pages (reports.rs:112-122).

GAPS
- Choosing pages: the app always exports every page (pdf.rs builds PdfOptions with PageSet::All); only novo_pdf::PdfOptions::pages and
  `kopercad-cli export pdf --page` can choose. The last step says so.
- No check kind / solve op for "a revision was issued"; the issue-submit step uses `clicked` (misses Enter).
- drawn_by / checked_by have no control (project_settings.rs); the sample project sets them.
- The PDF is read outside the app, so the checking steps are reading steps.
