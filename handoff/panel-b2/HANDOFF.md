# PANEL-B2 handoff

Resume: `cargo test -p novo-layout -p novo-app && cargo clippy --workspace --all-targets -- -D warnings`.

- `swing::sweep(enclosure, surfaces, door_devices, plate_devices, settings)` returns a `SwingReport` (hits with first-touch angle, bisected to 0.01 degrees; `unknown_depth`; `swept`). Door opens outwards about the hinge-side inner wall edge; `SweepSettings::hinge_offset` moves the axis outside the wall.
- Truth: with the axis at the wall edge, door devices never move towards the plate, so plate devices only matter when already touching at 0 degrees (rule `door.closed`). Real catches are the strike-side wall and (with an offset) the hinge side. The test checks the strike-side angle against the closed-form value.
- The door leaf itself is not swept; walls are 2 mm slabs.
- `CabinetLayout.door_devices` is empty until slice 4 fills it from the `surface` property; the sentences appear in the Door and depth panel (`cabinet_door::swing_sentences`) and the Cabinet header problem count includes hits.

- Plate devices without a depth cannot be obstacles: `SwingReport.unchecked_plate` lists them and the panel says so instead of claiming a clear sweep.
- Slice 1 has no UI path that puts a device on the door; the slice that fills `door_devices` from the `surface` property (roadmap item 4) adds it.

Slice 2 (#773): `openings::Opening` parses `opening.<name>` = `"<surface> round D X Y"` / `"<surface> rect W H X Y"` plus optional `keep K` and `to <surface> X Y`. `conflicts()` gives `OpeningConflict` (rule `opening.keepout`, main plate only), part of `CabinetLayout::report()` and the device problem rows. `best_passage()` ranks pass-through openings by in-surface path length; novo-routing does not call it yet. Unreadable properties and unknown surface names are `CabinetLayout.opening_problems`, shown in the Door and depth panel and counted in the Cabinet header.

Slice 3 (#782): `surface_rules::SurfaceRules` ("door: H S P", longest code wins), `split`, `plan_surface` (not called by the app yet). Devices sent to another surface are listed in the Place all message but not stored anywhere.
