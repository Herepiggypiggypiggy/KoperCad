# Handover package: handoff

Branch: claude/project-thread-033vce (kopercad). One PR from main, no stacking. Follow CLAUDE.md.

## State
Engine, command, menu entry and tests are in the PR. Tests: `cargo test -p novo-reports --lib handover`, `cargo test -p novo-app --lib handover`.

## Where things are
- crates/novo-reports/src/handover.rs: `handover_summary`, `truth_rows`, `revision_table`. Pure; takes facts from the app in `HandoverInput`.
- crates/novo-app/src/app/handover.rs: `export_handover[_to]`. Each file reuses an existing export (`export_pdf_choice_to`, `write_commissioning_report`, `write_safety_report`, report PDFs, `xlsx_bytes`).
- Command `export.handover` in novo-workflow commands.rs, menus.rs (File > Export reports), language.rs (nl), dispatch in app/workbench.rs.

## Traps
- The contents page is written last so it lists what really got written.
- Truth rows say "Exact" only for what is read from the drawing; wire lengths split by `is_estimate(basis)`; heat is always "Not included".
- Does not touch Workshop pack (#1175) or print preview (#1169) code; coordinate there before wiring them in.
- Files of an earlier package in the same dated folder are replaced.
