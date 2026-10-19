# AML import geometry: handoff

State: branch `claude/project-thread-2vinnl` in kopercad (PR for the importer); private repo `eplan-crane-configurator` branch `claude/aml-interruption-symbols` (interruption points as symbols, see below).

How it works
- `crates/novo-interop/src/aml/read.rs` reads `Wire` runs nested in nets (`PlantSegment`) and `Kind` `interruption` symbols.
- `draw.rs`: `design()` fits a library symbol to the file's pins (`fit_pins`: 8 orientations, uniform scale, pin faces must match) else a box; `draw_runs()` joins runs into wires.
- `novo-editor/src/art.rs`: `LibraryArt` scales artwork to instance pins and draws `import/box`.

Resume: `KOPERCAD_CRANE_AML=<real .aml> cargo test --release -p novo-cli --test aml_geometry -- --nocapture` (real export: 745 wires, 0 crossings, 1 open end that EPLAN itself leaves). Pictures of an import: `cargo run --release -p novo-cli --example aml_pages -- file.aml out`.

Traps
- Real pictures stay under `interop/private/aml-geometry/`, never in a public PR.
- The exporter (`scripts/kopercad_export.py`) is owned by the exporter thread; it already writes runs, junctions, texts, lines, rectangles and the symbol matrix. The interruption-point symbols (a `Kind` `interruption` symbol per EPLAN point, named after its pair) are the one addition from this lane.
