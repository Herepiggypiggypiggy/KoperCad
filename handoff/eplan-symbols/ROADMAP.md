# EPLAN-SYMBOLS roadmap

Scope: map EPLAN devices (function name, IEC 81346-2 tag letters, pin designations, optional symbol key) to KoperCAD symbols and back, as our own data tables. Never copy EPLAN's symbol library or macros; the numeric symbol keys come from the user's own export.

Done
- `novo_interop::eplan::symbols`: table (184 rows, every IEC and NFPA symbol), mapper both ways, generic box fallback, user symbol table (learn / save / load), mapping report into `novo_interop::report::Report`.
- Tests (`crates/novo-interop/tests/eplan_symbols.rs`): library coverage for both packs, round trip for every symbol, every table name resolves to its family, pin-count disambiguation, report counts.

Next
1. EPLAN-PXF-READ calls `SymbolMapper::map_all` for each device of the import and shows `MappingReport` in the preview (File > Import > EPLAN project). The user symbol table is saved next to the user's settings.
2. `learn()` input from PXF-READ: one `EplanSymbol` per distinct symbol key of the user's project or library export (key, function name, pins).
3. EPLAN-PXF-WRITE uses `to_eplan` / `to_eplan_symbol` for function text, tag letter, pins and the user's key.
4. Grow names and pin sets when real sample projects show gaps (user-supplied, private).

Open questions
- Which EPLAN function-definition numbers (category/group/definition) the PXF carries is unknown to us without a sample; names and the user table cover it until then.
