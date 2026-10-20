# Handoff: maker models in the 3D view
Code: `crates/novo-app/src/app/view3d_models.rs` (`Models::plan/collect/apply/counts`), wired in `view3d.rs` (scene build) and the Layers menu line in `view3d_controls.rs`.
Flow: on each scene build, devices on the plate whose project part matches a part with a STEP record in the user's parts library get a `GeometryJob` (size = the item's size) on a `GeometryService` (shared cache with the Parts window). `collect` runs every frame, moves ready STEP meshes into the view's `MeshLibrary` and bumps `revision`, which makes the scene build again with `Shape::Part`. Not usable models keep the box or typical shape.
Limits: no per-part rotation override yet; fixture `test_data/pins_box_8_holes.step` is not a closed solid, so it looks open from inside.
Resume: `cargo test -p novo-app view3d_models`.
