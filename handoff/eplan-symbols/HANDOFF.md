# EPLAN-SYMBOLS handoff

Branch `claude/eplan-symbols-c943p5` from main `e680ce7`. PR: see PROGRESS.md.

State: engine and tests done; no app UI of its own. The entry for the user is the import preview of EPLAN-PXF-READ (File > Import > EPLAN project), which owns the dialog and calls this API. Crate: `novo-interop` (added dependencies `novo-symbols`, `serde`, `toml`; no workspace line changed).

Files
- `crates/novo-interop/src/eplan/symbols/table.toml`: the data. One `[[entry]]` per KoperCAD symbol (pack-less path), `letters`, `names` (en, nl, de, first name is the one written on export), optional `pins` alias set. When the symbol library grows, `every_symbol_of_every_pack_has_a_row` fails until a row is added.
- `mod.rs`: `EplanSymbol` in, `Resolved` out (`Basis`: UserTable, Name, Letters, Generic), `SymbolMapper::{map, map_all, to_eplan, to_eplan_symbol, learn, gaps}`, `MappingReport::write_to(&mut Report)`.
- `pins.rs` pin matching (same designation, alias, `<name>2` for a repeated PE, then by position); `generic.rs` the box; `user.rs` `SymbolKey` and `UserTable` (TOML, ships empty).

Rules decided
- A named device nobody recognises stays generic; letters are used only when there is no function text (a weighing amplifier with letter A and pins 1 2 must not become a relay output).
- A one word table name inside a longer text counts only as the head noun (`red lamp`, not `weighing cell amplifier` for `cell`).
- Several equally good symbols: first table row wins, `ambiguous` is set and counted in the report.
- Equal names for pole-count families (breakers, contactors, fuses): pins decide; with no pins the first row.

Resume: `cargo test -p novo-interop --test eplan_symbols`; threads: PXF-READ (new), findings thread cse_01VQS5EbJ8bqSb6DPZ72Kuyw, reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.
