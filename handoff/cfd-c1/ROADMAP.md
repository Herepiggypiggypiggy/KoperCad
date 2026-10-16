# CFD-C1 meshing: roadmap

Scope: turn a `novo_thermal::Scenario` into a graded Cartesian grid with cell classification (crate `novo-cfd`, module `mesh/`). Pure functions, no solver code.

Done
- Graded axes (geometric growth, hard neighbour-ratio bound by splitting), planes at walls, device/obstacle faces, opening edges.
- Cell classification with exact volume shares (cut-cell lite), material ids, `MixedCell` list.
- Presets 50k / 500k / 2M cells by `MeshSettings::for_fidelity`.
- `MeshPreview` (counts, sizes, ratios, memory, `summary()` line) for the UI.
- Tests in `crates/novo-cfd/tests/mesh.rs`.

Next
- Nested/coarsened levels for grid sequencing if CFD-E needs them (today `locate` maps points between independent grids).
- Parallel classification (rayon) if profiling at 2M+ cells shows it matters.
- Resolve the plate slab with its own plane when CFD-C2 asks for it.

Open questions
- `ObstacleKind::Solid` maps to `MaterialKind::Wall`; a `Partition` kind would be clearer (CFD-A owns `result.rs`).
- Opening planes in z (depth) are not added: the scenario does not say where in depth a wall opening sits.
