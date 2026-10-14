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

## Next, in order
1. Drive #38 through CI and the design reviewer (cse_0114emQ5fSFrzcALbR4S18TU). After READY: no more pushes to #38.
2. Follow-up PR from main once #28 (canvas) merges: implement `SymbolArt::labels` (artwork text such as M 3~, pin names) and `tag_anchor` (from novo-symbols anchors) in art.rs.
3. Follow-up once #35 (WS-02) merges: potentials via `AssignPotentials` / `SetWirePotential` (feeds only at sources); tag texts via `AddText{TextContent::DeviceTag}` + `MoveText` (J11 text selection, drag, R, reset); drop own PE/N/PEN inference for the netlist's.
4. Once WS-08's novo-workflow (#29) merges: replace catalog search with `insert::InsertSearch::search` + `insert::child_queries` (keep the reading whose parent exists) or `palette::Palette::search_in(Mode::Place, ..)`; tags via `insert::next_tag(&Project, &structure, &Symbol)`; children via `insert::is_child_function`; use `duplicate::duplicate`, `renumber::{plan, apply}`, `keymap::Keymap::resolve(chord, Focus)`. Delete catalog/naming duplicates. Split agreed with WS-08.
5. J4 renumbering (project scheme, page based -3K1), renaming a coil renames contacts, Ctrl+D smart duplicate (with WS-08), highlight linked objects (with WS-10).
6. Budget tests: count input events per journey against WORKFLOW.md budgets (asked by workflow thread).

## Open questions
- Revised J3/J10/terminal criteria from the workflow thread (potentials defined at source; terminals suggested at location boundaries) — waiting for revised WORKFLOW.md / DESIGN.md.
- Until DESIGN.md merges: engines and commands only, UI stays thin.
