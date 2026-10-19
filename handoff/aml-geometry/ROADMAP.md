# AML import geometry: roadmap

Scope: the drawn page of an AutomationML import (pins, symbols, wires), not its texts (sibling thread, #1043).

Done
- Symbols fitted to the file's pins (orientation and scale), boxes sized from the pin sides, single-pin black boxes drawn as device pins.
- Wires from the file's `Wire` runs (exporter), joined into wires with bends and junctions; older files keep the net path.
- Art follows instance pins in `LibraryArt` (scale about the origin, leads for the rest); `import/box` art in `novo-editor`.
- Geometry tests `crates/novo-cli/tests/aml_geometry.rs`; dev tool `examples/aml_pages.rs`.

Next
- Page lines and rectangles the exporter now writes (location boxes, cable frames) are not drawn yet.
- EPLAN symbol matrix (`Matrix` on a symbol) is exported; the importer picks orientation from pin faces instead. Use it for symmetric symbols if one turns out mirrored.
- Power-pole contacts and motors are boxes; a 3-pole library symbol would look better.
