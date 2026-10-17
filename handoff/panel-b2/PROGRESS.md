# PANEL-B2 progress

- 2026-10-16: slice 1 (door swing sweep, layout field, panel lines, tests/swing.rs, state test in cabinet_tests.rs). fmt, clippy -D warnings and novo-layout/novo-app tests green.
- 2026-10-16: slice 2 (openings, #773) and slice 3 (surface rules, #782) READY; #773 merged with main.
- 2026-10-17: drill rows for openings on side walls and extra plates. `drill::Surface` is `Plate | Door | Named(name)`; `CabinetLayout.surface_sizes` gives each surface's size; `plate_drawing::documents` adds one sheet per used surface. Find it: Cabinet, plate drawing export (PDF/DXF) and the assembly pack drill table.
- 2026-10-17: #752, #773, #782, #876, #889, #893, #896, #900 merged (swing, openings, surface rules, door devices, drill rows, Place all door plan and preview, routing through openings).
