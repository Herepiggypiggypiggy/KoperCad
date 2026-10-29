# Drill plan roadmap

Scope: printable hand drilling plan per plate (FINAL-PRIORITIES item 3). Generic code, invented fixtures only.

Done: engine (`novo-layout::drill_plan`), A4 PDF (`novo-pdf::drill_plan`), Cabinet > Output > Export menu, command `export.drill_plan`, Workshop pack file, docs in `docs/PLATE-DRAWING.md`.

Next, in order:
1. Parts XML importer thread sets part property `drill_basis = maker` when it reads hole patterns/cut-outs from maker data (so rows say "maker data").
2. CLI `kopercad-cli export-drill-plan`.
3. Wire-range and strip length (item 4) are separate.
4. Optional: hole order by travel (serpentine) if the shop asks.

Open: duct fixing diameter (4.5 mm) and pitch are planning rules; replace with maker data when ducts have parts.
