# WS-06 Schematic editor: roadmap

Crate: `crates/novo-editor` (owner WS-06). Spec: docs/WORKFLOW.md (PR #24), refined by docs/DESIGN.md once merged.
Done criterion: the Gate 1 circuit can be drawn in under 5 minutes (about 73 actions).

## Done (PR #38)
- Headless engine: catalog search (mnemonics, library aliases, "K1 no" parent, recents, favourites), auto tags (IEC 81346 letter codes, page structure), EN 50005 contact numbers.
- Orthogonal routing (<= 2 bends), auto-connect (series insert, facing pins), T-junctions derived from topology (never placed).
- Move / rotate / mirror with rerouting, delete with junction tidy-up, rename with conflict check.
- Potentials via feed symbols; suggestions per wire count ("Three phase 400 V" first); PE/N/PEN inference.
- Pages: next page name (3 after 2, 1.1 between), add_page copying format and structure, Page Up / Page Down.
- Tool state machine: sticky tools, Escape ladder, Enter repeats, status hints, previews.
- Thin egui view `SchematicView` (I search, F2 rename, right-click menu), LibraryArt from WS-05 artwork.
- `show` returns the frame's ChangeSet for the shell's storage path.

## Done after #38
- #62 `coil K1` gives the existing device its coil; #70 `art` feature; `set_style` via #53.
- Proposals follow DESIGN.md §5 (#80): highlight follows pointer, Tab holds (24 px), solid highlighted proposal with hint, Delete declines. `edit::place` refuses a second main symbol (scripts too).

## In the follow-up PR after #91
- Proposals dashed per #92 (1 px / 2 px full strength), colours from the canvas style; Delete declines only with the pointer on the proposal; declined proposals remembered (`connect.declined`).
- novo-symbols `Connection.optional` (FORMAT.md); spare PE clamps optional; model_pins copies Pin.optional (#82 note 3).
- LibraryArt `labels` (artwork text such as M 3~) and `tag_anchor` from the library (design note N6).

## Next, in order
1. Letter codes (lead's call, DESIGN.md via #92): contactors and relays K by default, MCB F, MPCB Q, overload F; the editor already does this (catalog test). Still to do: the "Contactors are coded [K | Q]" project rule. Contactor and relay share the coil symbol, so the rule needs a contactor marker (part, or the device's main contacts) and a project-rule store; agree with WS-08 and WS-02.
2. Move catalog search / tags onto novo-workflow (#29): `InsertSearch`, `next_tag`, `duplicate`, `renumber`, `keymap`; delete the editor's duplicates.
3. #35 potentials (Wire.potential override, AssignPotentials) and tag texts (AddText DeviceTag, MoveText).
5. Reviewer notes: `no KM1`/`no M1` prefer the device reading when that device exists; `pt100` reads as device -PT100; proposal_holds should check crossings and bodies; deleting a device tidies its pages; the I popup becomes the command surface's place mode.
7. Budget tests per DESIGN.md §6 journeys.

## Open questions
- Revised J3/J10/terminal criteria from the workflow thread (potentials defined at source; terminals suggested at location boundaries) — waiting for revised WORKFLOW.md / DESIGN.md.
- Until DESIGN.md merges: engines and commands only, UI stays thin.
