# Visual redesign lane V3 (rail and flyouts): roadmap

Scope (docs/design/visual-redesign.md sections 3.2 and 14): the left tool rail replaces the dock in every workspace.

- V3a rail core and Schematic rail (this thread, PR 1): widget `crates/novo-app/src/app/rail.rs`, Schematic data `rail_schematic.rs`.
- V3a part 2: integrate V2a (top bar takes undo/redo, rail foot cluster `HISTORY` is removed), hints already live in the status bar.
- V3b 3D rail, V3c Cabinet rail, V3d Reports and Commissioning rails: later threads; each adds a data table and a `*_rail_show`.
- Add-on rows: `novo_addons::Extensions::rail_rows` into the last zone (max 6 per zone), not built yet.
Open: persisting the expand choice per workspace (today one setting `rail_expanded`).
- V3b 3D rail and V3c Cabinet rail: done in one PR (rows run existing commands; toolbars stay until V7). Open: 3D side panel entries, Cabinet Heat flyout, remove duplicate toolbar buttons.
