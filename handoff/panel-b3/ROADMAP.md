# PANEL-B3 roadmap: Cabinet UI for surfaces

Scope: surface picker in the Cabinet header, 3D view shows surfaces, collision and swing marks, Issues list shows rule violations. Reviewer R3. Types from novo-layout surfaces.rs/collide.rs (PANEL-B1); door-swing types from PANEL-B2 once its first PR merges.

Done
- Slice 1: `app/cabinet_surfaces.rs` summary model (rows, device counts, problems), header Surface picker, `cabinet.surface` command (View > Workspace > Cabinet surfaces).

Next, in order
1. Surface view: draw the picked surface's own plate (placements filtered by the `surface` property).
2. 3D view: surfaces as translucent panes (new novo-view3d file, one registration line; coordinate with 3D view and 3D look threads).
3. Collision marks (collide::collide) in 3D and the Issues list.
4. Swing marks and rule violations once PANEL-B2 types merge.
5. Place on a surface from the inspector and by drag.
