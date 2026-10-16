# FU-WIRE handoff

State: PR 1 pushed from `claude/project-thread-pda7tq` (see PROGRESS.md for the head SHA). Reviewer R2, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

Where the user finds it:
- Cabinet > Routes panel: "Wire rules" (collapsed) edits the rules; duct tint, outline and label on the plate; route table lengths (unrouted wires have no row and no length).
- Reports hub Cut list and Wire production, File > Export wire cut: use the same rules.
- `kopercad-cli export wirecut file.novo -o cut.csv`.

Key files: crates/novo-routing/src/project_rules.rs, model.rs (`wire_diameter_mm`, `wire_section_mm2`, `route_project`), crates/novo-reports/src/{lengths,cutlist,wirecut}.rs, crates/novo-app/src/app/cabinet_routes.rs, crates/novo-cli/src/lib.rs (`export_wirecut`).
Tests: crates/novo-reports/tests/wire_rules.rs, crates/novo-app/src/app/cabinet_routes_tests.rs, crates/novo-cli/tests/wirecut_parity.rs.

Resume: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`. No push after READY; follow-ups from main in a new PR.
