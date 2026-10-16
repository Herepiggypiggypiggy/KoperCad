# PANEL-B2 handoff

Resume: `cargo test -p novo-layout -p novo-app && cargo clippy --workspace --all-targets -- -D warnings`.

- `swing::sweep(enclosure, surfaces, door_devices, plate_devices, settings)` returns a `SwingReport` (hits with first-touch angle, bisected to 0.01 degrees; `unknown_depth`; `swept`). Door opens outwards about the hinge-side inner wall edge; `SweepSettings::hinge_offset` moves the axis outside the wall.
- Truth: with the axis at the wall edge, door devices never move towards the plate, so plate devices only matter when already touching at 0 degrees (rule `door.closed`). Real catches are the strike-side wall and (with an offset) the hinge side. The test checks the strike-side angle against the closed-form value.
- The door leaf itself is not swept; walls are 2 mm slabs.
- `CabinetLayout.door_devices` is empty until slice 4 fills it from the `surface` property; the sentences appear in the Door and depth panel (`cabinet_door::swing_sentences`) and the Cabinet header problem count includes hits.
