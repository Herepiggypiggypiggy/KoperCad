# PANEL-B3 roadmap: Cabinet UI for surfaces

Scope: surface picker in the Cabinet header, 3D view shows surfaces, collision and swing marks, Issues list shows rule violations. Reviewer R3. Types from novo-layout surfaces.rs/collide.rs (PANEL-B1); door-swing types from PANEL-B2 once its first PR merges.

Done
- Slice 1: `app/cabinet_surfaces.rs` summary model (rows, device counts, problems), header Surface picker, `cabinet.surface` command (View > Workspace > Cabinet surfaces).

- Slice 2: picked surface drawn from its own layout (`surface_layout`), view only; edits refused with a message.

Next, in order
2. 3D view: surfaces as translucent panes (new novo-view3d file, one registration line; coordinate with 3D view and 3D look threads).
3. Collision marks (collide::collide) in 3D and the Issues list.
4. Swing marks and rule violations once PANEL-B2 types merge.
5. Place on a surface from the inspector and by drag.

## Status 2026-10-16 23:50 UTC
Done: slice 1 (picker), slice 2 (view a surface, #823), slice 3 (3D panes, #874), slice 4 (put a device on a surface: Inspector > On surface; this PR).
Next, after PANEL-B2 slice 4 (#876) is on main: swing marks in 3D from `swing_report().hits` and swing rows in the Issues list (novo-checks door.rs, ProblemKind::DoorSwing with `SwingHit::message`). Device overlaps on the plate already show in 3D (Mark::Collision) and Issues (DeviceCollision). Inspector forms for the shown surface's size: later.

Slice 7: a device on the shown surface can be dragged (Drop), turned and taken off in the Cabinet; plan, rails, plate size and the enclosure stay plate-only. Door devices are not dragged in 3D (their position is edited where the door is shown).
