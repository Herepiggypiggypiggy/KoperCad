# Roadmap: part connection points (Parts library lane d)

Scope: where a wire meets a part. STEP to connection points, matching to catalog terminals, positions in the project part. Brief: docs/AMBITION-REVIEW.md brief 1(d). Schema: docs/PARTS-LIBRARY-DESIGN.md section 5 (`PartTerminal::position_mm`).

Done (PR on top of schema v2 #512):
- `novo_interop::part3d::pins`: scan a STEP model for wire openings (round and rectangular) and pins, in the frame of the render mesh; `map_terminals`; `apply_to_part`.
- `novo_parts::pins`: positions as project properties (`pin.<marking>.x_mm`...), `terminal_position`.

Next, in order:
1. UI: "Detect connection points" in the Parts window 3D model block (3D part geometry thread owns the block), with a per-terminal list and Measured/Partial/Estimated badges; manual nudge.
2. Lane lead: one line in `CatalogPart::to_model` calling `novo_parts::write_terminal_positions`.
3. Routing: `novo-routing` `anchor_of` uses `terminal_position` when the part has it (falls back to the footprint edge middle).
4. 3D view: draw terminal markers; wire ends snap to them.
5. Named terminals: match STEP solid or face names to markings; blade (faston) and DIN-rail clip detection; macro pin mapping (macro device to part terminals by marking).
6. Real maker files: run on user-imported models, tune the size limits (openings 0.8 to 14 mm).

Open: whether catalog terminal order is always left to right within a side (assumed).
