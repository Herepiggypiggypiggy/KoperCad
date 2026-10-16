# Xref readability handoff

- `novo-xref`: `Label.rows` (`LabelRow`: contact symbol, kind, pins, location from the coil's page) next to the text `lines`; `Label::extra_lines` are the warnings.
- `novo-canvas::note_table`: pure layout of the table (glyphs, columns, density). `SymbolNote.rows/density` feed it; `PageScene::add_note` draws it, so canvas and PDF match.
- App: Preferences > Cross references > contact table density (`Settings::xref_density`, "full" default).
- CLI PDF export uses the table at default density.
