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

## Next, in order
1. Letter codes per DESIGN.md (#80): contactor Q, mpcb Q, mcb F, with a per-class rule. Cross-crate (novo-workflow `insert::letter_code`, Gate 1 tests use -K), so agree with WS-08 and the lead first.
2. Move catalog search / tags onto novo-workflow (#29): `InsertSearch`, `next_tag`, `duplicate`, `renumber`, `keymap`; delete the editor's duplicates.
3. #35 potentials (Wire.potential override, AssignPotentials) and tag texts (AddText DeviceTag, MoveText).
4. SymbolArt labels/tag_anchor (#28) in art.rs.
5. Reviewer notes: `no KM1`/`no M1` prefer the device reading when that device exists; `pt100` reads as device -PT100; proposal_holds should check crossings and bodies; deleting a device tidies its pages; the I popup becomes the command surface's place mode.
6. Single-click select under Xvfb (box select works): check with WS-03, then on Windows.
7. Budget tests per DESIGN.md §6 journeys.

## Open questions
- Revised J3/J10/terminal criteria from the workflow thread (potentials defined at source; terminals suggested at location boundaries) — waiting for revised WORKFLOW.md / DESIGN.md.
- Until DESIGN.md merges: engines and commands only, UI stays thin.
