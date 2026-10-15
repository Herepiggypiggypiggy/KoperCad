# Gaps found while writing the Terminals, PLC and documentation lessons

Found by checking every step of the ten draft lessons against the app code (nothing was run in a window). Each lesson's `NOTES.md` has the per-step file:line. Lessons stay `draft` until the gap behind them closes. Owners are suggestions.

## Missing UI (the lesson step is written, marked `# GAP:`)

| Gap | Lessons | Suggested owner |
| --- | --- | --- |
| Cable proposals have no command id or key; Enter (`proposal.accept`) takes terminal proposals only; cable proposals are not drawn on the canvas | cables-and-shields | Workflow, UI integration |
| No control to set a cable's part, length, type or shield (engine has `assign_cable_part`, `add_shield`, `connect_shield`); Accept passes no catalog | cables-and-shields | Parts, UI integration (cable inspector) |
| Wire numbering: no rule in `ProjectRules`, no renumber; `edit.renumber` and `project.rules` show "Comes later"; F2 on a wire does nothing; an unnumbered wire cannot get a number | wire-numbers | Schematic editor, Workflow, WS-02 |
| Drawer core table has no wire-number column (data exists, `screen.rs:40`) | wire-numbers | Cables drawer |
| Assigning a part to a device has no UI (inspector Part is read-only) | wire-list-and-bom | Parts, Inspector |
| `export.bom` is listed but has no handler; no Excel or CSV export in the app (only in `kopercad-cli` and `novo_reports`) | wire-list-and-bom | Outputs (WS-13), app shell |
| PDF export always uses `PageSet::All`; the app cannot choose pages; no UI for drawn by / checked by | pdf-export | Outputs (WS-09), app shell |
| Terminal drawer actions (accept, jumper, PE, move strip) are buttons or menu items with no command ids; F2 does not rename a terminal; `xref.link` does not pair interruption points across pages | terminal-strips, interruption-points | Workflow, Editor, novo-xref |
| `add card` draws no card symbol and offers no argument chips; PLC channels show no address, signal text or cross reference on the drawing; no inspector chip; `plc.signal` has no editor | plc-io | PLC (WS-17), app shell |
| "Switch placed symbols" control for the drawing standard does not exist (engine `novo_workflow::switch::plan` does); the editor loads only the IEC catalog, so NFPA keys draw as placeholder boxes | drawing-standard | App shell, Editor, Canvas |
| No list or marker for symbols left without a twin | drawing-standard | App shell, Checks |
| Generated pages are added only when a project is opened; no command adds them later (a first PLC card in a session shows no I/O list until reopen) | cables-and-shields, plc-io | Outputs, app shell |

## Check kinds and solve operations the lessons wish for
`terminal = { strip, name }`, `terminals_at_least`, `jumper`, `pe`, `page_kind`, `interruption_pair`, `revision = { index }`, `sheet = "10b"`, `rule = { standard }`, `live = { pole_closed, output_on }`, cable `part`/`length`/`shield_connected`, `field` with `equals`, potential name on a wire. Solve: `rename`, `accept_suggestions`, `jumper`, `toggle_pe`, `move_to_strip`, `cable` with wire pairs, `cable_part`, `cable_shield`, `set_potential`, `wire_number`, `assign_part`, `issue_revision`.

## Anchors
No anchor registry exists yet, so every `ui` id is a proposal. Ids used are listed in each lesson's `NOTES.md`; shared ones: `outline.pages`, `drawer.cables.tab`, `drawer.cables.accept`, `drawer.terminals.tab`, `palette.field`, `live.card`, `live.pole`, `live.output`, `mark.menu.export`, `revision.submit`.

## Bugs and oddities seen
- Opening a project with frozen pages adds no generated pages (`reports.rs:114`); issue a revision after the generated pages exist.
- Adding a device without a part renames BOM sheet 10 to 10a and 10b.
- `Page.frozen` doc says "revision or an export"; only a revision freezes.
- `to plc` routing retraces 4 mm of the lamp lead and can cut through the DQ box for a sideways signal.
- Standard switch: radio click and switch are two undo steps; `switch::plan(.., Iec)` writes `"iec"` while the radio writes `""`.
- A 2-wire signal loop is proposed as a numbered `3x1.5`, never shielded or 2-core; the library letter code for a PLC input is K.
- Setting a feed's potential leaves the old potential as an orphan.
- Outline row reads "40  Terminal diagram · +CAB-X1" (report title mixed with the strip tag); PageUp/PageDown reach generated pages slowly.
