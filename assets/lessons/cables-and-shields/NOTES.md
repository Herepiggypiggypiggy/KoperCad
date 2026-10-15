# cables-and-shields: reviewer notes

Teaches: wires that leave the cabinet become cables; the C tool; accepting proposals; reading cores; where a part, length and shield go. Status `draft` (two GAP steps).
Project: pump station (motor + level transmitter in +FLD, terminal strip and PLC input in +CAB), built with `novo_editor::edit::place/draw_wire`. `finished.json` = C tool on the transmitter wires (`proposal_for_wires`), Accept (`propose_cables`), part via `novo_parts::assign_cable_part`, length, shield via `add_shield` + `connect_shield(Pe)`. Builder and tests: `crates/novo-tests/tests/lessons/cables.rs`.

| Step | Verified path (file:line) |
| --- | --- |
| open-drawer | Ctrl+4 `keymap.rs:392`, `commands.rs:593`, handler `workbench.rs:513`; tab `drawers.rs:62-66` |
| read-proposals | count `drawers.rs:51`; button `drawers.rs:73-79` (no hover text); empty text `drawers.rs:185-190` |
| cable-tool | C `keymap.rs:348`, `commands.rs:274-280`, `workbench.rs:459`, `tool.rs:696`; hint `tool.rs:2544`; dock has no Cable button `dock.rs:89` |
| draw-line | `tool.rs:1109-1121` clicks, Enter `view.rs:1259-1261`, `tool.rs:1730-1762`; `app.rs:799-805` -> `drawers.rs:128-150` (undo step "Draw cable") |
| accept | `drawers.rs:153-172` (undo step "Accept cables") |
| read-cores | core grid `drawers.rs:224-262`; colours `novo-cables propose.rs assign()` |
| free-core | list `drawers.rs:198-222`, "n of m" `drawers.rs:217` |
| part-and-length, shield | GAP, no control (see below) |

GAPs
- Accepting cable proposals has no command id and no key; Enter / `proposal.accept` only accepts terminal proposals (`workbench.rs:370`). Proposals are not drawn on the canvas (only counted in the drawer). Declining a cable proposal (`dismiss_proposal`) has no control either.
- No control sets a cable's part, length or type, adds a shield or connects it (engine: `assign_cable_part`, `add_shield`, `connect_shield`). Inspector knows devices only (`inspector.rs:78`); the drawer is read-only. Accept passes no catalog, so accepted cables never get a part (`drawers.rs:158`).
- The generated Cable diagram page only appears when a project is opened (`app.rs:549-554`); no command adds generated pages later.
- Check kinds wished for: `field` on a cable (part, length), `cable` with `shield_connected`; solve operations `cable_part`, `cable_shield`.
Oddities: a PLC channel symbol has letter code K, so it would be tagged `-K2` next to the contactor; the sample types `=P1+CAB-A1` by hand. The 2-wire loop gets a numbered `3x1.5` cable (no 2-core type is proposed for signals).
