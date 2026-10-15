# Handoff: panel part data

Branch `claude/project-thread-qxuwc2` (kopercad), draft PR "Part power loss, mounting and clearances". Needs the main reviewer; Parts library thread (cse_01Vba4yMwZ8vHadcR2jj9wFD) owns novo-parts, coordinate via the integration lead.

Decisions
- Stored as free-form part properties, no new core type (WS-02 untouched). Device property overrides the part's, field by field (`PanelData::of_device`, `layout::model::device_margins`).
- Catalog format 1 -> 2 is a migration of legacy key names and mounting text only.
- Clearance sides are in the part's upright frame; a rotated device turns them 90 degrees clockwise.
- `plan.rs` (place all) still spaces by the default gap only; R7 will use margins.
- Layout crate duplicates the key strings (it does not depend on novo-parts), like the width/height keys.

Resume: `cargo test -p novo-parts -p novo-layout`.

UI: `app/cabinet_panel.rs` is a section for the Cabinet inspector (power loss, mounting, four clearances): `show(ui, fields, project, id) -> Option<PanelEdit>`, sent as `Ask::SetPanelValue`; `command()` validates (via `PropertySheet::edit`) and gives the one-undo-step command. The right panel itself belongs to UI integration (cabinet), PR #443 (`cabinet_inspector.rs`); it calls the section inside its device block. Until #443 is on main the section has no host (dead-code allows mark it) and the values are not editable in the app. Not done: the full property sheet / Parts window (UI integration: parts), per-side gaps on the canvas (still one outline).
