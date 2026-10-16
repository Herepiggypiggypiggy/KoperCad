# CFD-C1 meshing: handoff

Branch `claude/cfd-c1-meshing-tcr1wx`, repo kopercad, PR from main. Reviewer R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`.

Code: `crates/novo-cfd/src/mesh/` (`axis.rs` 1D grading, `regions.rs` scenario to boxes and materials, `classify.rs` `Mesh` and exact shares, `preview.rs`, `mod.rs` settings and `plan_grid`). Tests: `crates/novo-cfd/tests/mesh.rs`.

Model decisions
- Frame as the stub: mm, x right, y down, z from the plate (z=0, rear wall) towards the door. Plate is a slab 0..plate_thickness; devices stand on it; obstacle height = blockage x air depth.
- Box faces thinner than two near-size cells do not make planes (cut cells instead). Planes closer than a quarter near cell are merged.
- Overlaps resolve by priority: devices, obstacles, plate.
- Conductivities in `MaterialInfo` for devices/duct/rail/partition are planning values; CFD-C2 owns the real ones.
- Cell count is found by bisection plus a local scan, typically within 5 percent, tested within 10.

Resume: `cargo test -p novo-cfd --test mesh`, `cargo clippy --workspace --all-targets -- -D warnings`.
