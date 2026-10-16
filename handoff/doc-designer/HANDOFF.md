# DOC-1 handoff

Branch `claude/project-thread-bdmgix` in kopercad, from main b025e2f. First PR carries the whole engine (types, render, storage, PDF hook, tests). No novo-core change, no CLAUDE.md edit, no new crate.

Resume: `cargo test -p novo-reports --test designer`, `cargo test -p novo-pdf --test designer_pdf`. After an intended render change regenerate the golden with `UPDATE_GOLDEN=1 cargo test -p novo-pdf --test designer_pdf` and review the diff of `crates/novo-pdf/tests/golden/designer_device_list.txt`.

Where the code is: `crates/novo-reports/src/designer/` (template, fields, table, render, validate, store, builtin, raster), hook `export_layouts_pdf` in `crates/novo-pdf/src/report.rs` (the report PDF writer now shares `write_sheets`).

Decisions: templates never store rows (live data only); unknown fields print as typed and are listed; the frame and title block default to the canvas geometry (10 mm frame, 140 x 36 mm block); `page`/`pages` fields are set per page by the renderer; PDF output has no footer for designer pages.

Not done: the editor UI and the menu/command registration (only `designer::ENTRY` exists), writing templates into a project from the app, barcodes and images, label printer sheets.

Contacts: reviewer R2 and integration lead A (cse_01FiLvRBKEcNNHbQEGhYyL1S); roster in coordination/threads.md.
