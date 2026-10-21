# CFD and cable ducts: handoff

What was wrong (2026-10-21, Tadjiro's report):
- The bridge gave every duct `blockage: None`, so the 3D mesh made it 0.7 x the air depth tall (about 200 mm on a 300 mm cabinet; real ducts are 25 to 80 mm). That tall block was the pierced slice and part of the CFD error.
- `heatview::cfd::solid_at` counted any material id other than air as solid, so the slice, arrows, streamlines and the probe treated the porous duct as a wall.

What changed:
- `novo-thermal` bridge: duct blockage = Rail.height / (interior depth - plate thickness); an unsized duct uses its width and is reported (`BridgeReport::unsized_ducts`).
- `novo-heatview`: ducts are air in `solid_at`.
- `novo-cfd`: `CabinetSolver::with_duct(DuctModel)`, `DuctModel::closed()`, a result note about ducts.
- Study: `crates/novo-cfd/tests/duct_compare.rs` (prints `DUCTSTUDY` rows).

Limits: duct drag values are planning numbers; wire heat is not modelled; the 2D engine uses the same blockage (now the real height).
Evidence: /mnt/project-files/qa/duct-before and duct-after (scenario 42-3d-heat).
