# CFD-G handoff

Branch claude/cfd-g-validation-9oow4h. Files: crates/novo-cfd/tests/validation/mod.rs (helpers), validation_conduction.rs, validation_cavity_study.rs, docs/thermal-validation.md.
Resume: `cargo test -p novo-cfd --test 'validation_*' -- --nocapture`, update the table in the doc.
Reviewer R1 cse_01NCdhCktKEbKPHFBvi2Tjsp; lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.
Finding: backward Euler is first order in time (transient slab 1.2% at tau/400).

## State 2026-10-17
Report covers chimney, flow core, plate, conduction, sealed enclosure, vented box, device and full cabinet. Owed: own chimney check against the 2D formulas and a cabinet-with-vents validation (separate small PRs). When CFD-C2 moves validation_vented to the wall-weighted metric, re-check the vented rows.
