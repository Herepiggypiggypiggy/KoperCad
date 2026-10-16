# Relations preview handoff

- `novo-app/src/app/symbol_preview.rs`: pure `symbol_preview` (placed symbol shapes and pins from the model and any `SymbolArt`), `fit`, egui `paint`. Reuse it for the Shift+G list.
- `inspector.rs`: `Relation` carries `what`, `pins`, `page`, `preview`; `relation_card` draws the card; the whole card jumps.
- `workbench.rs`: `preview_art` caches the library art once.
