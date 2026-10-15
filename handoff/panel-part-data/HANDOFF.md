# Handoff: panel part data

Branch `claude/project-thread-qxuwc2` (kopercad), draft PR "Part power loss, mounting and clearances". Needs the main reviewer; Parts library thread (cse_01Vba4yMwZ8vHadcR2jj9wFD) owns novo-parts, coordinate via the integration lead.

Decisions
- Stored as free-form part properties, no new core type (WS-02 untouched). Device property overrides the part's, field by field (`PanelData::of_device`, `layout::model::device_margins`).
- Catalog format 1 -> 2 is a migration of legacy key names and mounting text only.
- Clearance sides are in the part's upright frame; a rotated device turns them 90 degrees clockwise.
- `plan.rs` (place all) still spaces by the default gap only; R7 will use margins.
- Layout crate duplicates the key strings (it does not depend on novo-parts), like the width/height keys.

Resume: `cargo test -p novo-parts -p novo-layout`.

UI status (honest): there is NO app UI for these values yet. `novo-app` does not depend on `novo-parts` and shows no property sheet on `main` (checked 2026-10-15). The editing model is ready: `PropertySheet::build(project, &[device])` lists `power_loss_w`, `mounting` (with choices), `clearance_*_mm` as editable rows under "Part data"; `sheet.edit(project, &RowKey::PartData(key), text)` returns the command. The cabinet canvas already draws the clearance outline from the largest side. Missing: the panel that hosts the sheet (UI integration: parts), and a cabinet inspector showing the per-side gaps (UI integration: cabinet). Asked both threads.
