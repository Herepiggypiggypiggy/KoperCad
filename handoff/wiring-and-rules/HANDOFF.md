# Wiring and place rules: handoff

- Rules panel: `crates/novo-app/src/app/cabinet_rules_panel.rs` (cards, plain sentences, ? help); window glue and Save/Undo in `cabinet_rules.rs`. Edits apply to Place all at once; Save writes `place-rules.txt`. "Edit as text" keeps the old text editor.
- Wire guide: `Editor::wire_step`, `tool::free_pins`, `SchematicView::paint_wire_guide` (novo-editor). Scenario `qa/scenarios/1850-wiring-guide.qa`.
- The surface (door/plate) rules still have no editor.
