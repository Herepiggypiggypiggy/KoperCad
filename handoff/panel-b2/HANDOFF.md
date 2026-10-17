# PANEL-B2 handoff

Resume: `cargo test -p novo-layout -p novo-app && cargo clippy --workspace --all-targets -- -D warnings`.

- `swing::sweep(enclosure, surfaces, door_devices, plate_devices, settings)` returns a `SwingReport` (hits with first-touch angle, bisected to 0.01 degrees; `unknown_depth`; `swept`). Door opens outwards about the hinge-side inner wall edge; `SweepSettings::hinge_offset` moves the axis outside the wall.
- Truth: with the axis at the wall edge, door devices never move towards the plate, so plate devices only matter when already touching at 0 degrees (rule `door.closed`). Real catches are the strike-side wall and (with an offset) the hinge side. The test checks the strike-side angle against the closed-form value.
- The door leaf itself is not swept; walls are 2 mm slabs.
- `CabinetLayout.door_devices` is filled by `model::load` from the `surface` property (slice 4, #876); the sentences appear in the Door and depth panel (`cabinet_door::swing_sentences`) and the Cabinet header problem count includes hits.

- Plate devices without a depth cannot be obstacles: `SwingReport.unchecked_plate` lists them and the panel says so instead of claiming a clear sweep.
- Devices go on the door with the Cabinet Surface picker or Place all with surface rules.

Slice 2 (#773): `openings::Opening` parses `opening.<name>` = `"<surface> round D X Y"` / `"<surface> rect W H X Y"` plus optional `keep K` and `to <surface> X Y`. `conflicts()` gives `OpeningConflict` (rule `opening.keepout`, main plate only), part of `CabinetLayout::report()` and the device problem rows. `best_passage()` ranks pass-through openings by in-surface path length; novo-routing calls it (#896). Unreadable properties and unknown surface names are `CabinetLayout.opening_problems`, shown in the Door and depth panel and counted in the Cabinet header.

Slice 3 (#782): `surface_rules::SurfaceRules` ("door: H S P", longest code wins), `split`, `plan_surface`.

Slice 4 (#876): `model::load` fills `CabinetLayout.off_plate` (device -> surface name) and `door_devices` from the placement `surface` property; `on_plate()` is what the plate checks, `swing_report()` and `preview()` use. An unknown surface name, or a door where the enclosure has none, stays on the plate.

Drill table (#889): each opening is a numbered `CutoutRow` "Opening <name>" on the plate or door sheet, centre in the surface's own mm; openings on other surfaces give a problem line.

Place all (#893, #900): the surface rules' devices are planned with `plan_surface` (`cabinet_rules::surface_plans`, `CabinetState.surface_preview`); Enter stores plate plus surface plans in one step; the door view draws the door part before Enter. Needs a desktop look in the experimental build.

Routing (#896): `route_connections` reads `off_plate` and `openings` once per enclosure. Plate to door: plate leg through the ducts to the best opening, plus the door leg and a loop (`Routed.surface_legs`); door to door: straight free-air run; no opening: unrouted. A model: the gap between plate and door and the hinge loop are not in the length. `best_passage` is an approximate choice (straight-line ranking).

Open: openings on side walls and extra plates in the drill table; a lesson or tooltip for surface rules; measuring the plate-to-door gap for wire lengths.
