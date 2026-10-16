# PANEL-B1 handoff

Resume: `cargo test -p novo-layout && cargo clippy --workspace --all-targets -- -D warnings`.

API (all pure, millimetres, enclosure.rs 3D frame: x across, y down, z back wall to door)
- `surfaces::Surfaces::from_enclosure(&Enclosure, &Plate, &[(name, "w h margin [x y z]")])` returns the surfaces plus `SurfaceProblem`s (never silent). `find(Some(name))`, `None` is the main plate.
- Placement property `surface` names the surface; structure property `surface.<name>` carries its size (`surfaces::size_key`). No novo-core change.
- `Surface::device_body(rect, depth, mesh)` gives a `collide::Body`: the mesh, or a box flagged `is_fallback()`.
- `collide::collide(a, b)` (penetration, touching is fine, containment counted for closed meshes), `clearance(a, b)`, `keeps_clearance(a, b, mm)`. Results carry `approximate` when a box fallback was involved.
- `Body::rotated_y(px, pz, angle)` is the hook for the door swing sweep (PANEL-B2).

Decisions
- Coplanar faces touch, they do not collide (solid faces on the same plane).
- Containment is sampled (256 triangle centres) and assumes closed meshes.
- Extra plates without an origin are placed beside the previous plate and marked `assumed`.
- Plate is centred in the interior across width and height (planning value).

No UI yet: PANEL-B3 owns the Cabinet surface picker and 3D marks.
