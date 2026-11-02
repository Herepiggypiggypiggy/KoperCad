# Wiring and place rules: progress

- 2026-11-02: rules cards panel, wire guide and free-pin rings built; tests in cabinet_rules_panel.rs and novo-editor/tests/wire_guide.rs.
- 2026-11-02 (PR 2): clickable Corners/Lines switch on the wire guide (asks the shell for `wire.mode`, which owns the setting); door/surface rule cards in the Rules panel (`Ask::SetSurfaceRules`, one undo step, "Back to KoperCAD's default" clears the property); the Rules panel now uses the canvas height. Scenarios 1852/1855 (shape switch), 1853/1854 (surface cards). Still open: preview of the auto route before the end click, a "why was my click refused" popover.
