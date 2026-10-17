# ui-pdf-options handoff

Branch `ui-pdf-options`, one PR from main. Code: `crates/novo-app/src/app/pdf_options.rs` (dialog, range parsing, mapping), `pdf.rs` (`export_pdf_choice_to`, `pdf_path`), `settings.rs` (`PdfChoice`, saved per user by eframe storage), command `export.pdf_options` in `novo-workflow` commands, menus (File > Export drawings), language (Dutch).

One-click Export PDF and Export this page as PDF are unchanged and always use `PdfChoice::default()`.
Tests: `cargo test -p novo-app pdf_options`.
Trap: settings are RON (eframe), not JSON.
