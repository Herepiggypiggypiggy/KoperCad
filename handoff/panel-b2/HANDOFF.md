# PANEL-B2 handoff

Resume: `cargo test -p novo-layout -p novo-app && cargo clippy --workspace --all-targets -- -D warnings`.

Slice 1 (#752, merged or READY): `swing::sweep` returns a `SwingReport` (first-touch angle per door device and obstacle, bisected to 0.01 degrees; `unknown_depth`; `unchecked_plate`). Hinge axis at the wall edge unless `SweepSettings::hinge_offset`. The door leaf itself is not swept. `CabinetLayout.door_devices` is empty until a slice fills it from the `surface` property.

Slice 2: `openings::Opening` parses `opening.<name>` = `"<surface> round D X Y"` / `"<surface> rect W H X Y"` plus optional `keep K` and `to <surface> X Y`. `conflicts()` gives `OpeningConflict` (rule `opening.keepout`), part of `CabinetLayout::report()` and of the device problem rows in the Cabinet. `best_passage()` picks the opening with the least extra wire between two surfaces; nothing in novo-routing calls it yet. Unreadable properties land in `CabinetLayout.opening_problems` (not yet shown in the UI).
