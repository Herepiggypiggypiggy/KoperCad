2026-10-16 17:51 UTC slice 1 written: surface summary model, header picker, cabinet.surface command, 3 tests
2026-10-16 18:09 UTC R5 review on 510c595 addressed: popup id asserted in test, plate-only message, MAIN_PLATE
2026-10-16 19:52 UTC #757: merged main again (cabinet_bar::group caption fix, CI red), R8: problems and surface note now wrapped full-width lines
2026-10-16 19:09 UTC #757 merged main (menu now View > Cabinet), head a40ece7; slice 2 surface view coded (now branch claude/panel-b3-surface-view-2 from main after #757 merged)
2026-10-16 UTC #823 R7 blocker: surface view now refuses SetPlate, SetPlateSize and SetEnclosure as well (view only message), a missing surface falls back to the plate with a visible warning, tests extended. R5 note: inspector plate/door forms still edit the mounting plate while a surface is shown; later slice.
2026-10-16 UTC #823 merged. Slice 3: surfaces as faint panes in the 3D view (View3D toolbar > Layers > Surfaces; the Cabinet-picked surface is highlighted), branch claude/panel-b3-3d-surfaces.
2026-10-16 UTC #874 R5: added app test that the panes land on the real shell (door pane = door ghost). Finding: the scene's enclosure box starts at the plate while the walls reach plate_setback behind it. No GPU adapter in cloud: screenshot of picked door and roof needs a desktop run.
2026-10-16 23:50 UTC slice 4: Inspector 'On surface' row (Ask::SetSurface, cabinet_surface_move.rs); plate view hides devices mounted elsewhere; 5 tests in cabinet_surface_move_tests.rs. Needs a desktop look.
2026-10-17 UTC #876 merged. Swing hits of door devices are now Issues rows (novo-checks door.rs, ProblemKind::DoorSwing, point at the device). Next: 3D marks for the devices involved.
2026-10-17 UTC slice 7: Drop/Rotate/Remove allowed on a shown surface; 3D drag skips off-plate devices; cabinet_surface_edit_tests.rs.
