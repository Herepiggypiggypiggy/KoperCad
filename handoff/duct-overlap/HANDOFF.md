# Duct overlap: handoff

Rule: two ducts may touch edges (T or corner: one ends against the side of the other) but never cover each other.

- Detection: `novo_layout::duct_overlaps` reports `DuctHit::Duct`; helpers `ducts_in_the_way`, `clear_duct_spot`, `model::duct_clash`.
- Place all: row ducts start at the trunk's side (`plan_ducts` in `novo-layout/src/rules.rs`); two kept-apart ducts only when both fit.
- Editor: `cabinet_rails.rs` (`duct_refusal`, `new_rail_at` snaps and shortens), `view3d_move.rs` (group move).
- Issues: `novo-checks/src/physical.rs`, kind `DuctOverlap` ("Duct over duct, rail or device").
- Lesson JSON: `cargo run -p novo-app --example panel_lessons -- assets/lessons`, `plc_panel_lesson`, and `UPDATE_LESSONS=1 cargo test -p novo-app --lib emc_demo`.
