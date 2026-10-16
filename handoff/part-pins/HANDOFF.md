# Handoff: part connection points

Branch `claude/project-thread-igzjif`, stacked on #512 (schema v2, head 70a39ae). Lane lead: thread "Parts library at scale" (owns catalog.rs and schema.rs). 3D part geometry thread: branch claude/project-thread-5cps74-3d (owns the Parts window 3D model block).

State: engine and tests done, draft PR. Nothing is half done.

Where the code is:
- `crates/novo-interop/src/part3d/pins.rs`: `scan_step`, `map_terminals`, `apply_to_part`, `Fix`.
- `part3d/mesh.rs`: `PartXf` and `derive_xf` (the same axes/mirror/flip as `finish`).
- `part3d/tess.rs` `Soup.features`, `build.rs` collects them per face.
- `crates/novo-parts/src/pins.rs`: property keys and readers.
- Tests: `crates/novo-interop/tests/pins.rs` (synthetic STEP writer in the test file).

Method: planar faces with inner loops of 0.8 to 14 mm (aspect up to 3.5) are openings; convex cylinders r 0.3 to 3 mm that reach the outline are pins. Direction comes from the nearest outline face, not from the face normal. Faces toward the mounting plate are dropped. Matching is per side in catalog order (top/bottom left to right, left/right top to bottom, front row by row).

Traps: no real maker file has been tried (maker sites are blocked from cloud). Screw heads or vents with the wrong size can pose as openings; the count rule flags them as Partial/Estimated. `Side` has no Back.
Do not edit catalog.rs or schema.rs until the lane lead says so.

Follow-ups for a fresh session (not started, usage limit):
1. Parts window 3D model block (branch claude/project-thread-5cps74-3d, `crates/novo-app/src/app/parts.rs`): button "Detect connection points" calling `scan_step` -> `map_terminals` -> `apply_to_part(.., false)`, list per terminal with Measured/Partial/Estimated, save catalog.
2. `CatalogPart::to_model` in `crates/novo-parts/src/catalog.rs` (lane lead's file): add `crate::pins::write_properties(self, &mut properties);` (public as `novo_parts::write_terminal_positions`).
3. `crates/novo-routing/src/model.rs` `anchor_of`: when the device's part has `novo_parts::terminal_position(&part.properties, marking)`, use it (plate x/y from placement plus the part x/y; mind rotation) instead of the footprint edge middle.
PR #526 base is #512's branch; retarget to main after #512 merges.
