# 3D scene model: handoff

State: merged (PR #391). Code: `crates/novo-view3d/src/build/{mod,model,tests}.rs`, `crates/novo-view3d/tests/build.rs`. Feature `model` is on by default.

API: `build::from_layout(&CabinetLayout, &Inputs, &Options) -> Scene` (pure), `build::model::scene(&Project, StructureId, &Options) -> Option<Scene>` (reads the project), `build::reaching_door(&Scene)`, `build::refresh(&mut Scene, Scene)` (bumps `revision` / `pose_revision`). Rebuild fully on every edit.

Conventions (3d-view.md section 4): Y up via `PlateFrame::world`, z from the plate surface towards the door, plate at z<0, item origin at the middle of the back face. Rail 35 x 7.5; a device on a rail has its back at z = 7.5. A rotated device keeps its unrotated size and turns -90 degrees about +Z. Unknown depth (60 mm) and duct section (40 x 60) are drawn at the default and marked `GuessedSize`. Wires are `Scene::wires` (radius = diameter/2) at half the shallowest duct height or 15 mm in free air; free-air wires carry `Mark::Outside`. The door is a translucent box in hinge group `DOOR_HINGE`. Mark priority: Collision, Outside, TooClose, GuessedSize (only the top one shows).

## Next steps (not started; usage limit)
1. Read the enclosure from R2 once PR #409 (`novo_layout::model::enclosure_of`, `novo_layout::enclosure::{Enclosure, Door, Hinge}`) merges: replace `Shell` (depth 250 mm, left-hinged door) in `Options`, take the door hinge side, opening angle and depth from it, and use #409's `device_depth` instead of `build::model::device_depth`.
2. Align the unknown-duct default: `DEFAULT_DUCT_MM` is 40 x 60 here, 30 mm wide in novo-layout (#461). Pick one and share the constant.
3. Convert `novo-routing` routes to `WireInput` when the app has wire jobs.
4. Expose `reaching_door` as a `novo-checks` Problem; document that only the top mark shows (a guessed device in a collision shows no guess edge).

## What the user sees
Nothing directly. It feeds the 3D face of the Cabinet canvas, which the renderer thread (`novo-view3d` widget, PR #481) and "UI integration: cabinet, 3D, enclosure" (session_01G52sVY5N34jPsrkqWukApR) wire.
