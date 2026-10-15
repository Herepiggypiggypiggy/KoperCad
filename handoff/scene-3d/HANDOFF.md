# 3D scene model: handoff

Branch `claude/project-thread-p3jp6n`, draft PR #391, based on the skeleton branch of PR #439. Code: `crates/novo-view3d/src/build/{mod,model,tests}.rs`, `tests/build.rs`.

Conventions (contract, section 4): Y up via `PlateFrame::world`, z from the plate surface towards the door, plate at z<0, item origin at the middle of the back face. Rail 35 x 7.5; a device on a rail has its back at z = 7.5. A rotated device keeps its unrotated size and turns -90 degrees about +Z. Unknown depth (60 mm) and duct section (40 x 60) are drawn at the default and marked `GuessedSize`. Wires are `Scene::wires` (id = wire id, radius from the diameter), at half the shallowest duct height or 15 mm in free air; free-air wires carry `Mark::Outside`. The door is a translucent box in hinge group `DOOR_HINGE`; the angle sign gives outward swing for both sides.

Assumptions to revisit: enclosure interior equals the plate; depth is 250 mm and the door a default until PR #409 (`novo_layout::enclosure`) lands.

What the user sees: nothing directly; it feeds the 3D face of the Cabinet canvas, wired by the renderer and "UI integration: cabinet, 3D, enclosure" threads.

Resume: read `build/mod.rs` docs, then `docs/design/3d-view.md` section 5.
