# Roadmap: STEP-1 geometry follow-ups

Scope: `crates/novo-interop/src/part3d/` except `pins.rs`. Plan: plans/spawn-table.md (lane STEP-1, reviewer R2).

Done (PR 1): B-spline surface tessellation (`nurbs.rs`, `tess.rs`), spindle torus fix, forced refine passes for tori.

Next, in order:
1. PR 2 from main: per-part rotation override (`PartRotation`, `geometry_from_step_rotated`, rotated cache key, `GeometryService::request_rotated`) and generated shapes (`generated.rs`: DIN rail profiles, wiring duct).
2. The Parts window "Detect connection points" button belongs to lane PINS-UI (owns parts.rs, needs the PARTS-Q hook line); see HANDOFF.

Open: swept and revolved surfaces still fall back to flat triangles; uniform/quasi-uniform B-spline forms without knot lists are not read.
