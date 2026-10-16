# Roadmap: STEP-1 geometry follow-ups

Scope: `crates/novo-interop/src/part3d/` except `pins.rs`. Plan: plans/spawn-table.md (lane STEP-1, reviewer R2).

Done:
- PR 1 (#570): B-spline surface tessellation (`nurbs.rs`, `tess.rs`), spindle torus fix, forced refine passes for tori.
- PR 2: per-part rotation override (`rotation.rs`, `geometry_from_step_rotated`, `cache_key_rotated`, `GeometryService::request_rotated`) and generated DIN rail / duct shapes (`generated.rs`).

Next, in order (other lanes, listed so nobody forgets):
1. The app adopts `request_rotated` and `generated::{din_rail, duct}` (view3d scene and the Parts window "3D model" block); needs a stored rotation field on the part (catalog, WS-07 / PINS-UI).
2. PINS-UI: "Detect connection points" button in `novo-app/src/app/parts.rs`; pin positions must follow the rotation, use `PartRotation::apply_to_point(size, p)`.

Open: swept and revolved surfaces still fall back to flat triangles; uniform / quasi-uniform B-spline forms without knot lists are not read; TS15 and G rails are not generated.
