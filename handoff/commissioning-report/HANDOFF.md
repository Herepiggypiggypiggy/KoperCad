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

## How to reach it (entry points)
Engine is on PR #404 (READY). UI entries, not built yet, requested from the UI threads (2026-10-15 18:45 UTC):
- Command "Export commissioning report..." (palette, File/Export menu): PDF, CSV, xlsx. Owner of menu slot: UI integration: reports and checks (session_01TaTTPuTqhk8bHd277oJRFR).
- `kopercad-cli commissioning --pdf/--csv` (novo-cli).
- "Touch layout" toggle (View menu, palette, chip in the commissioning view header). Owner of placement: Workspaces, menus and navigation (session_01WAQSEGkMbyreCPG18tsSPc).
Blocked on: `to_report_input` from novo-commissioning (PR #431, written after #404 merges).
