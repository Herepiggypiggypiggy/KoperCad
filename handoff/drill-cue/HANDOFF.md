- PR 2 (branch claude/drill-cue): drill cue on the Cabinet plate. When an edit adds screw holes (device hole pattern, rail screws) a ring closes on each new hole, the bore opens to its true diameter and fades (0.9 s, max 24 cues). Code: `app/drill_cue.rs`, hook in `cabinet.rs`. Not drawn on first load, other surfaces, with Reduce motion or under the harness. 3D plate version is not built yet.
- Not run here: the QA harness (box lacks openbox, vulkan, xkbcommon); unit tests, fmt and clippy pass.

## 3D plate cue (PR drill-cue-3d)
The 3D view reads the same plate holes per project revision (first read only learns) and draws the same rings and bores as overlay lines 9 mm above the plate, so rail screws under a DIN rail still show. Devices hide the holes beneath them (depth tested). Off with Reduce motion and under the harness; `KOPERCAD_MOTION=hold` freezes the cue for pictures (scenario 307). Cost: while a cue runs the overlay list is re-set and the window repainted at about 30 Hz for 0.9 s.
In 3D the bore is the hole's true radius; only the closing ring is kept at 6 mm or more for legibility. The colour is `ui_palette().info`, the same as the Cabinet cue.
