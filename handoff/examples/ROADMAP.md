# Example gallery roadmap

Scope: larger example projects with a full cabinet (File > Examples), drawn by code through the editor commands (`crates/novo-app/src/app/example_kit.rs`), one file per example.

Done
- Kit (`example_kit.rs`), registry and tests (`example_gallery.rs`, `example_gallery_tests.rs`: Check clean, no duct overlap, heat inside limits, BOM without gaps, command opens it).
- Motor control centre (`example_mcc.rs`): 3 DOL feeders, 1 star-delta feeder, 24 V DC control, 5 pages, plate with rails and ducts.

Next, one PR each from main (no stacking)
1. Crane control cabinet: 2-speed hoist with spring brake, two travel drives, 48 V AC control, radio steps. Generic public knowledge only, never plans/crane-setup-facts.md.
2. VFD pump station.
3. PLC/IO cabinet with 24 V supply.

Open: door-mounted operators are placed on the plate by the planner; the field motors show as Info "not placed".
