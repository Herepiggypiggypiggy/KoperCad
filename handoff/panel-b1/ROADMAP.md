# PANEL-B1 roadmap: surfaces and collision

Lane B of panel layout intelligence, first slice. Plan: `handoff/panel-layout/HANDOFF.md` (Lane B plan).

1. Done: `novo-layout/src/surfaces.rs` (plate, door, walls, roof, floor, profiles, extra plates; property convention) and `collide.rs` (mesh SAT, BVH, clearance, box fallback).
2. PANEL-B2 (next): door swing sweep with `Body::rotated_y`, wiring-hole openings, per-surface Place all, in rules/plan/drill/enclosure.
3. PANEL-B3: Cabinet UI surface picker, 3D view marks, Issues rows.
