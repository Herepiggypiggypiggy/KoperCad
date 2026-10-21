# Windows pass J small fixes: handoff

- `crates/novo-app/src/app/pdf.rs`: `pdf_base()` picks the PDF folder; every KoperCAD PDF ends with a `%KoperCAD-project: <16 hex digits>` comment after `%%EOF` (FNV-1a hash of the project path, never the path itself: PDFs go to customers); `replaceable_path_for` only replaces a PDF that names the same project file (or none), else `<stem> 2.pdf`.
- `crates/novo-canvas/src/widget.rs`: `fit_scene` whole-sheet branch; `settle_fit` runs in the caller and still follows canvas size changes (test added).
- Not verified on Windows.
