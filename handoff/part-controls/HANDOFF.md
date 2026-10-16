# Handoff: part and cable controls

Branch claude/part-and-cable-controls-d8vf18. New files: crates/novo-app/src/app/part_picker.rs (picker list, quality badge, assign command, "Use for selected device"), cable_controls.rs (Part, Length, Shield rows). Hooks: inspector.rs Part row, drawers.rs cables_tab, parts.rs one button and action, workbench.rs two fields.
Reviewers: R2 cse_01SEokxUZ1MKDwyQvLm4DPys, R3 cse_01SL6nnpsSpN8Tjt3tfo8K5V, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.
Decisions: shield connects at the cabinet end only (model records one connection); "From layout" length is the longest routed core wire (cabinet run only).
