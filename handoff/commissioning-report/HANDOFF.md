# Commissioning report and tablet layout: handoff

Thread session: cse_015JWsgz1ouocSSfvtqPic6V. Branch: claude/project-thread-n01i61 (repo kopercad).
Sibling (data model, tick-off flow): cse_01Gpw3BVv45jMLL8rVjwtg4P. Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: `crates/novo-reports/src/commissioning.rs` has `CommissioningInput` (checks, punch items,
sign-offs), progress per cabinet and page, `readiness`, and `commissioning_report` giving a `ReportOutput`
(summary, cabinets, pages, checks, punch, signoff). CSV works through the existing `to_csv`.

Interface proposed to the sibling: they provide a mapping from their records to `CommissioningInput`.
Agreed by the sibling: they provide `to_report_input(&Project, &CommissioningStore)` in the new crate novo-commissioning (design PR #399); wait for their message that it landed, then register the report.

Resume: `cargo test -p novo-reports commissioning`. `novo_pdf::export_report_pdf` (crates/novo-pdf/src/report.rs) writes any ReportOutput as a stand-alone PDF.
Next: export command (menu, CLI), then the large-target tablet layout in novo-app.
