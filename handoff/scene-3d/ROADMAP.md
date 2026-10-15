# 3D scene model (thread): roadmap

Scope: turn the cabinet data (enclosure, plate, rails, ducts, devices, routed wires, door) into the `novo-view3d` scene of `docs/design/3d-view.md` section 5, with correct depths, and keep it in sync with edits. Code: `crates/novo-view3d/src/build/` (feature `model`, on by default). No rendering.

## Done
- `build::from_layout` (pure), `build::model::scene` (reads the project), `build::reaching_door`, `build::refresh` (revision counters).

## Next
1. Replace `Shell` with `novo_layout::enclosure` (R2, PR #409) once it merges; use its `device_depth`.
2. Convert `novo-routing` routes to `WireInput` once the app has wire jobs.
3. Expose `reaching_door` as a check/Issue with the UI integration thread.
