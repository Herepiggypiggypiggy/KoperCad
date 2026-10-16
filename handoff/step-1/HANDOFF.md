# Handoff: STEP-1 geometry follow-ups

Repo herepiggypiggypiggy/kopercad, PRs from main. Reviewer slot R2, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S. Reviewer for #570 (570 mod 3 = 0): Main reviewer A.

State:
- PR 1 #570 (branch claude/project-thread-8sfst3): B-spline surfaces + spindle torus. Open, non-draft.
- PR 2 (branch claude/step-1-rotation): rotation override + generated DIN rail / duct. Independent of #570 (both touch mod.rs lines, trivial conflict at most).

Where the code is (all `crates/novo-interop/src/part3d/`):
- `nurbs.rs`, `brep.rs`, `tess.rs`: B-spline surfaces (`Surface::BSpline` marker, `Face::nurbs`, `Sf`, `unwrap_nurbs`, `spindle_branch`).
- `rotation.rs`: `PartRotation` (quarter turns about x, y, z in part space, applied after axes and mounting side, then re-seated), `geometry_key_rotated`. `mesh::finish_rotated`, `mesh::levels` (shared LOD tail).
- `cache.rs`: `cache_key_rotated` (unrotated key unchanged, rotated adds `rXX`), `cached_part_geometry_rotated`. `service.rs`: `request_rotated`; `GeometryJob` is unchanged on purpose (parts.rs builds it with a struct literal).
- `generated.rs`: `din_rail(&RailSpec, Quality)`, `duct(&DuctSpec, Quality)`, `RailProfile::from_name("NS 35/7,5")`. Origin is `MeshOrigin::Box` because adding a variant breaks the exhaustive match in `novo-app/src/app/parts.rs`. Slots / fingers drop out of the detailed level when over the triangle budget.
- Tests: `tests/part3d_surfaces.rs`, `tests/part3d_generated.rs`.

Traps: no maker file was tried (maker sites are blocked from cloud). Bump `ALGORITHM_VERSION` on any output change (now 3). Rail and duct slot sizes (5.2 x 25 at 25 mm; 5 mm fingers and slots, 2 mm walls) are planning values, not a maker's catalog.

Not done here: the "Detect connection points" button (scan_step -> map_terminals -> apply_to_part) is in `novo-app/src/app/parts.rs`, which the spawn table gives to PINS-UI (wave 2, needs the PARTS-Q hook line). See handoff/part-pins/HANDOFF.md.
