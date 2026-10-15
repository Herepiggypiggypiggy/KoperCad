# Handoff: panel part data

Branch `claude/project-thread-qxuwc2` (kopercad), draft PR "Part power loss, mounting and clearances". Needs the main reviewer; Parts library thread (cse_01Vba4yMwZ8vHadcR2jj9wFD) owns novo-parts, coordinate via the integration lead.

Decisions
- Stored as free-form part properties, no new core type (WS-02 untouched). Device property overrides the part's, field by field (`PanelData::of_device`, `layout::model::device_margins`).
- Catalog format 1 -> 2 is a migration of legacy key names and mounting text only.
- Clearance sides are in the part's upright frame; a rotated device turns them 90 degrees clockwise.
- `plan.rs` (place all) still spaces by the default gap only; R7 will use margins.
- Layout crate duplicates the key strings (it does not depend on novo-parts), like the width/height keys.

Resume: `cargo test -p novo-parts -p novo-layout`.
