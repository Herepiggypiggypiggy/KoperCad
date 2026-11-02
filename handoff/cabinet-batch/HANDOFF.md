# Cabinet batch select and drag: handoff

Branch `claude/project-thread-r9e7qd` (kopercad). Code: `crates/novo-app/src/app/cabinet_group.rs` (landing maths, nudge), `cabinet.rs` (Drag.group, Ask::MoveGroup, Ask::Arrange, strip, Ctrl+A, arrows), `view3d_align.rs` (`arrange_devices`). Tests: `cabinet_group_tests.rs` and unit tests in `cabinet_group.rs`.

Other thread: auto-placement tools (auto rail, ducts, autogroup, autodistance, reconfigure) live in their own files and the Cabinet toolbar; this PR deliberately adds no toolbar button (floating Align row (docked under the plate, not floating) instead) so the two do not collide.

Remaining friction in the cabinet workflow (found while reading, not fixed):
- Selecting a rail and a device at once is not possible; rails have their own selection.
- No copy of devices on the plate; the 3D view has Duplicate, the plate has none.
- Align/spread are not in the command palette or the right-click menu for the plate.
- Group R rotates each device in place, not the group.
- Moving a group does not warn about collisions during the drag (the boxes go red, no sentence).
- Toolbar is crowded and folds items into More at 1366 px.

Resume: `cargo test -p novo-app --lib cabinet_group`.
