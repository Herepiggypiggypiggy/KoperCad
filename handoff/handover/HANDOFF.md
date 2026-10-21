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
- Drawing set uses `settings.print_choice` (plain when default) and writes one PDF per plot set (`write_plot_set`, `plot_sets_with_pages` in print_preview.rs); Workshop pack goes to `Workshop/` via `export_workshop_to`.
- A rerun removes package-numbered files it no longer writes (names in `OWN_FILES`, `1 Plot set *`); other files stay.
- Files of an earlier package in the same dated folder are replaced.
- Ask before writing: `ask_handover` opens `HandoverAsk` when the checks have an error or warning; `export_handover_to` stays unconditional for tests.
- Parts list is "Unverified" (project parts have no review ledger link); wire list column is "Route length mm", the bench cut list adds the ferrule allowance.
