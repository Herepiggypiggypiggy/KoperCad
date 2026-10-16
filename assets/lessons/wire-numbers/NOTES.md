# wire-numbers: reviewer notes

Teaches, truthfully: a wire's POTENTIAL comes from its net (feed symbol, optional per-wire override); its NUMBER is a typed field. Reads both in the generated Wire list and Cable diagram. Status `draft` (numbering rule, renumber and first number have no UI).
Project: the pump station of `cables-and-shields` with its cables, the loop supply named `L+`, motor wires numbered 1, 2, 2 by hand, and the generated pages the program adds on open. `finished.json`: `set_feed_potential(+24 V)` and `Command::SetWireNumber` (the exact calls of the UI). Builder/tests: `crates/novo-tests/tests/lessons/cables.rs`.

| Step | Verified path (file:line) |
| --- | --- |
| open-wire-list, back-to-drawing, wire-list-again, cable-diagram | outline page list `app.rs:999-1100`, `show_sheet` `app.rs:874`; generated pages added on open `app.rs:549-554`, `reports.rs:109` |
| read-list | wire list columns `novo-reports wires.rs:191-206`, potential `:257-270`, cable core `:240-249` |
| set-potential | feed menu chips `novo-editor view.rs:1651-1700`, `potential_menu.rs:13-24`, `tool.rs:2370`, `potential.rs:399` |
| type-number | label drawn `novo-canvas model.rs:1604-1620`; double-click `view.rs:952-985`; box hint `view.rs:1562`; `SetWireNumber` `view.rs:1577-1585` |
| cable-diagram | `diagrams.rs:178-246` (Wire no. and Potential per core) |

GAPs
- No wire numbering rule (`ProjectRules` has none, `entity.rs:88-106`) and no numbering engine (`docs/design/macros.md:60`, `macro_place.rs:11`). `project.rules` and `edit.renumber` ("Renumber devices") are not in `APP_COMMANDS` (`workbench.rs:183-229`): Ctrl+K shows "Comes later". `plc.renumber_io` is I/O addresses. DESIGN.md promises a rule (lines 1030-1066).
- An unnumbered wire cannot get its first number: only an existing label can be double-clicked; F2 on a wire does nothing (`view.rs:1292-1308`) though DESIGN.md:276 says it edits the number; no wire row in the context menu; no property sheet in the app (`novo-parts sheet.rs:276` has the row).
- Potential label on a wire (`TextContent::Potential`) cannot be created from the UI; wire potential is only seen in the wire list, drawer and live view.
- No design check for duplicate wire numbers. The Cables drawer core table has no Wire no. column (`screen.rs:40` has the data).
- Check kinds wished for: `potential` (name on a wire's net), `field` on a wire (number); solve operations `set_potential`, `wire_number`.
