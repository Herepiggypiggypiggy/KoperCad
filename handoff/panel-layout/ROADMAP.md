# Panel layout intelligence: roadmap

Gap #4 of `docs/AMBITION-REVIEW.md` (PR #504). Two lanes, one thread, one PR at a time (restart the branch from main after each merge).

## Lane A: rules-based placement
1. [done, PR A] Engine: classes, ducts, explanations, rule check, rail and duct cut optimiser (`novo-layout`: `rules`, `check`, `stock`).
2. [done, PR A] App: Place all by the rules, `place-rules.txt`, Rules window, rule lines in the Cabinet inspector, Cut list window with trim-to-length. Next: cut list report and CLI, rule problems in the Issues list (after #486).
3. Rotation to fit, stacking small devices, vertical side ducts.

## Lane B: mounting surfaces and collision
1. Surfaces: plate, door, side walls, roof/floor, profiles; devices mount on a surface (stored as placement properties, no core change).
2. Mesh-level collision (triangle meshes from part geometry, #449), door swing sweep against plate devices, wiring holes, multi-plate cabinets.
3. 3D view shows surfaces; Cabinet UI picks the surface.
