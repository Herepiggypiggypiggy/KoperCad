# CFD-C1 meshing: handoff

Round 1 merged as #577. Round 2: branch `claude/cfd-c1-refine` (repo kopercad), PR to main, reviewer R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, merge lead B `cse_011NKUp2tgBn8uVWK6WHsc7k`. Related: CFD-C2 `cse_01957w91FMssXKAxLNw5h6iv` (PR #705 sheet plate, lumped devices), CFD-INT `cse_01T6DxyR7AahcmcfVSwop3Cx` (#691).

Code: `crates/novo-cfd/src/mesh/` (`axis.rs` 1D grading with `Span`s, `regions.rs`, `classify.rs` `Mesh` + `BodyResolution`, `preview.rs`, `mod.rs` `MeshSettings`, `plan_grid_report`). Tests: `tests/mesh.rs`, `tests/mesh_refine.rs`. Examples: `mesh_grid` (grid and body resolution per budget), `mesh_study` (solver run per budget; env NEAR GROWTH NEED THIN RESOLVE_THIN).

Decisions
- Budget order: bodies first (`min_cells_across` per axis; devices N, ducts/rails N-1, vents 2, plate edges as planes), then the coarse air size is bisected to hit the target. If the bodies do not fit, N is lowered (reported in `GridPlan::cells_across`); below 1 the plan is `over_budget`.
- Thin bodies (< `thin_mm` 8 mm, the 2.5 mm plate) stay in cut cells by default (`resolve_thin` false): a 2.5 mm layer under 40 mm cells made the pressure solve about 6 times slower and the cabinet tests did not converge. Measured with resolve_thin on: surface 123 to 102 C and plate share 0.77 to 0.68 over 1.5k to 12k cells (device k 2 W/mK, no contact resistance), so it is not an improvement before #705.
- Planes closer than min(0.15 x coarse size, 5 mm) are slivers and left to cut cells.
- Study of the reference sealed cabinet (50 W, 2D 82.3 C, plate share 0.27), pre-#705 solver: surface 73.9, 81.4, 82.0, 84.6 C at 1.5k, 3k, 6k, 12k cells; main's mesh gave 74.6, 82.5, 83.1, 84.3. Variants at 6k (need 1/2/3, near 0.2/0.35/0.6, growth 1.15/1.3) spread 78.3 to 84.3 C: the scatter is the solver's near-wall treatment, not the grid layout. Plate share reads 0 with the volume plate in a cut cell.

Resume: `cargo test -p novo-cfd --test mesh --test mesh_refine`, `cargo run --release -p novo-cfd --example mesh_study -- 3000 6000`.
