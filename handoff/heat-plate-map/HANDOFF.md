# Heat plate map: handoff
Cause: after a heat run the 3D view showed only the Slice plane (75 mm out by default) plus Cloud and Flow when switched on; the mounting plate itself was never coloured, so the cabinet plate heat was not visible in the crane sim example.
Fix: the cabinet solver now publishes its plate sheet temperature per rear column (`CfdSnapshot::plate_field_c`, NaN where no plate; `novo-cfd/src/cabinet/snapshot.rs`). `plate_mesh` in `crates/novo-app/src/app/heat_3d.rs` draws it on the plate face. Results without a plate sheet (stub, some reduced runs) draw no plate map.
Where to find it: Cabinet > Simulate heat (or 3D > Heat), then 3D, Plate switch next to Slice. Scenarios 1990 and 1990b (crane sim example) toggle Plate and use the new QA step `expect_shots_differ`.
It is the solver's plate sheet temperature (model, estimate), not the air next to the plate.

## Follow-up: any view angle, opacity, walls and door
- Back view: the overlay faces were never culled; from behind the plate body hid the map. `plate_mesh` now draws a second copy behind the plate body (z = -PLATE_THICKNESS_MM - 0.5), so it shows from either side.
- Opacity: default `SURFACE_OPACITY` 0.45 (was 235/255), slider in the Heat strip next to Plate, Walls, Door. Maps use draw order -2, the cloud -1, so the cloud blends over the maps instead of being painted over.
- Walls and door: `CfdSnapshot::wall_field_c` (novo-cfd, `FlowSolver::wall_surface_temps`) holds the inner surface temperature of all six faces per cell, solver face order. It is the solver's air next to the wall pushed through the shell panel (`u_panel`, inner film, sun offset), the same stack as the Cabinet shell report; the test `every_wall_carries_an_inner_surface_temperature...` pins the map mean to the report. Walls are not solved as solids: this is an estimate, and the tooltips say so. Rear wall is drawn only where the plate does not cover it. Walls switch is on by default, Door is off (it covers the view into the cabinet from the front).
- Not built: wall conduction between faces, window part of the door (door map uses the panel value), the rear wall behind the plate.
- Where to find it: Cabinet > Simulate heat (or 3D > Heat), then 3D; Plate, Walls and Door switches and the opacity slider next to Slice. Scenario 1991.
