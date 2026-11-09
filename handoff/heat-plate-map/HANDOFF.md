# Heat plate map: handoff
Cause: after a heat run the 3D view showed only the Slice plane (75 mm out by default) plus Cloud and Flow when switched on; the mounting plate itself was never coloured, so the cabinet plate heat was not visible in the crane sim example.
Fix: the cabinet solver now publishes its plate sheet temperature per rear column (`CfdSnapshot::plate_field_c`, NaN where no plate; `novo-cfd/src/cabinet/snapshot.rs`). `plate_mesh` in `crates/novo-app/src/app/heat_3d.rs` draws it on the plate face. Results without a plate sheet (stub, some reduced runs) draw no plate map.
Where to find it: Cabinet > Simulate heat (or 3D > Heat), then 3D, Plate switch next to Slice. Scenarios 1990 and 1990b (crane sim example) toggle Plate and use the new QA step `expect_shots_differ`.
It is the solver's plate sheet temperature (model, estimate), not the air next to the plate.
