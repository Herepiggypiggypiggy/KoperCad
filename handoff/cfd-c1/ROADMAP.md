# CFD-C1 meshing: roadmap

Scope: turn a `novo_thermal::Scenario` into a graded Cartesian grid with cell classification (crate `novo-cfd`, module `mesh/`). Pure functions, no solver code.

Done
- Round 1 (#577): graded axes, exact cut-cell shares, presets 50k/500k/2M, `MeshPreview`.
- Round 2 (branch claude/cfd-c1-refine): cells across every body and opening (`min_cells_across`, default 3), a budget ladder that lowers the number when the bodies alone exceed the target (`GridPlan`), planes at body faces kept unless they are slivers, ducts and rails one cell fewer, plate lateral edges as planes, faster growth inside solids, `BodyResolution` and `unresolved_bodies` in the preview, `plate_slab` and `resolve_thin` switches, `LevelTuning::mesh` to pass mesh rules to the cabinet solver, `examples/mesh_study.rs` and `mesh_grid.rs`.

Next
- Rerun `mesh_study` once CFD-C2's sheet/lumped device model (#705) is on main and add the mesh-independence test to `tests/validation_device.rs` territory (CFD-G owns the table).
- Near-wall air cells next to hot device faces are still set by `near_ratio` only; the laminar thermal boundary layer (about 2 mm at 60 K) is not resolved at 6k cells, so surface temperatures there depend on the solver's wall treatment.
- Nested levels for grid sequencing if CFD-E needs them (`locate` maps points between independent grids).

Open questions
- `ObstacleKind::Solid` maps to `MaterialKind::Wall` (CFD-A owns `result.rs`).
- Opening planes in z are not added: the scenario does not give the depth of a wall opening.
