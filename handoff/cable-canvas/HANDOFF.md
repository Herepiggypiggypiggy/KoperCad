# Cable on the schematic: handoff
Bug: in build 5 a drawn cable was invisible on the schematic. Cause: `PageScene` (novo-canvas/src/model.rs) never drew cables.
Fix: `sync_wire` adds a sheath primitive and tag label for wires with `cable_of_wire`; `apply` redraws core wires on Cable changes (`wire_cables` map). Test: `a_cable_is_drawn_over_its_core_wires_and_goes_with_it`.
PR #525, head 169757f, un-drafted. Not checked by eye in the running app.
Next: when merged, tell Release thread (cse_01UuGC6jT84yVrbcwGaSJ5Df).
