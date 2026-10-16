# Handoff: STEP-1 geometry follow-ups

Branch `claude/step-1`, PRs from main in `herepiggypiggypiggy/kopercad`. Reviewer slot R2, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: PR 1 (B-spline surfaces + spindle torus) done locally, tests green. PR 2 (rotation override, generated DIN rail and duct) not started.

Where the code is:
- `part3d/nurbs.rs`: `SplineSurface` (de Boor basis, eval, Gauss-Newton `project`).
- `part3d/brep.rs`: `Surface::BSpline` marker (Surface stays `Copy`), `Face::nurbs`, `Reader::spline_surface` reads simple and complex (rational) `B_SPLINE_SURFACE_WITH_KNOTS`.
- `part3d/tess.rs`: `Sf` bundles a surface with its B-spline data; `unwrap_nurbs` inverts boundary points and falls back to flat when the boundary is off the surface or crosses a seam; `spindle_branch` keeps a boundary through a spindle torus axis continuous; `refine` takes forced passes (B-spline 2, torus 2).
- Tests: `crates/novo-interop/tests/part3d_surfaces.rs` (synthetic faces and synthetic STEP).
- `ALGORITHM_VERSION` is 3 (caches rebuild).

Traps: no maker file was tried (maker sites are blocked from cloud). A flat-triangle mesh of a curved patch can overshoot the true area by several percent at the coarse tolerance; tests allow for that. Adding a variant to `MeshOrigin` breaks `novo-app/src/app/parts.rs`, so generated shapes must use `MeshOrigin::Box`. Do not add a field to `GeometryJob` (parts.rs builds it with a struct literal).

The "Detect connection points" button (scan_step -> map_terminals -> apply_to_part) is in `novo-app/src/app/parts.rs`, owned by PINS-UI (wave 2, needs PARTS-Q hook line merged). It is not part of this PR; see handoff/part-pins/HANDOFF.md.
