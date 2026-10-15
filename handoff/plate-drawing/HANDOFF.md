# Plate drawing: handoff

Branch claude/project-thread-vbfqaz, draft PR "Plate drawing and drill table (R5)" on Herepiggypiggypiggy/kopercad. Based on main 1f47882.

State: complete engine and both writers with tests; fmt, clippy -D warnings and tests of novo-layout, novo-pdf, novo-interop pass locally. No UI, no CLI.

Resume: `cargo test -p novo-layout -p novo-pdf -p novo-interop`. To see a sheet, write `export_plate_pdf(&docs, "name")` to a file and open it (see tests/plate.rs in novo-pdf for how docs are built).

Decisions: marks are neutral (novo_layout::plate_drawing) so PDF and DXF draw the same thing; annotation sizes are paper mm times the scale denominator; DXF origin is the plate top left with y up (table y is -y in the file); door sheet assumes door = plate size until R2; rail holes are 5.5 mm, 10 mm from the ends, at most 200 mm apart.

Traps: do not edit [workspace.dependencies]; novo-pdf and novo-interop now depend on novo-layout (already registered). Reviewer after push: message with head SHA.

Entry points (2026-10-15 19:15, #470): app palette command `export.plate` runs `NovoApp::export_plate_pdf()` (crates/novo-app/src/app/plate_export.rs), which exports the enclosure selected in the Cabinet canvas; CLI `kopercad-cli export plate FILE -o OUT.pdf [--enclosure TAG]`. Cabinet button and File menu item are for the cabinet UI thread (#443). Door sheet size from the enclosure: #477 (includes #409's commit).
