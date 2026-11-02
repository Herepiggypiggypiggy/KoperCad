# Cabinet batch select and drag: roadmap

Scope: make the Cabinet plate usable with many devices at once (request 3, first half).

Done (this PR)
- A drag that starts on a selected device carries the whole selection (one undo step, grid and DIN rail snap per device). A drag on an unselected device selects it first.
- Ctrl+A selects every device on the plate.
- Arrow keys nudge the selection by a grid step (Shift: 1 mm).
- Align strip (left, centre, right, top, middle, bottom, spread across, spread down) shows over the plate with two or more devices selected. Shares the maths with the 3D view (`view3d_align::arrange_devices`).
- Right-click inside a selection keeps it, so Rotate and Take off act on the group.
- Box select and Shift-click were already the canvas's own and still apply.

Next
- Copy and paste of a device group (needs a model decision: a copy is a new schematic device).
- Align/spread as registered commands in the palette and right-click menu on a multi-selection.
- Group rotate about the group centre (today R turns each device in place).
- Clamp a group drag to the plate edge.
- Rail-aware group drag preview (followers snap only on drop result, same as lead).
