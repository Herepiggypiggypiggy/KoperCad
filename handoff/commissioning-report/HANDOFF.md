# Commissioning report and tablet layout: handoff

Thread session: cse_015JWsgz1ouocSSfvtqPic6V. Branch: claude/project-thread-n01i61 (repo kopercad).
Sibling (data model, tick-off flow): cse_01Gpw3BVv45jMLL8rVjwtg4P. Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: `crates/novo-reports/src/commissioning.rs` has `CommissioningInput` (checks, punch items,
sign-offs), progress per cabinet and page, `readiness`, and `commissioning_report` giving a `ReportOutput`
(summary, cabinets, pages, checks, punch, signoff). CSV works through the existing `to_csv`.

Interface proposed to the sibling: they provide a mapping from their records to `CommissioningInput`.
No reply yet (see PROGRESS.md).

Resume: `cargo test -p novo-reports commissioning`. Next is the standalone report PDF in novo-pdf (the
existing exporter only prints project pages inside the schematic frame).
