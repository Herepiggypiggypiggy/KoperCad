# Heat H5 (thermal mass and heat-up time): roadmap

Scope: transient heat-up with the real heat capacity of walls, plate and devices; time constants; duty cycles and day/night ambient. Lane H5 of handoff/heat-research/ROADMAP.md.

Done:
- PR #1859 (merged): `novo-thermal::mass` (exact modal network solver, slab, phases, periodic state, analytic tests), `heatup::heat_up`, strip line "Wall and plate mass: 90 % after ... (model, not measurement)".
- PR 2: heat-up built on H1's shell (`Scenario::shell()`): layers per face, door window, frame bridge, plate setback gap, physical outer film via `outside::resolve`; `HeatUp::provenance` and `mass_check` (catalogue weight over modelled wall sheet); hover says assumed/catalogue.

Next, in order:
1. Wall and plate mass inside the 2D `simulate_transient` and the 3D transient frames (use `ShellModel::capacity_per_face_j_per_k` / `WallStack`), so the animation itself slows down.
2. Duty-cycle and day/night controls in the Heat-up strip (engine takes `Phase` lists).
3. Sun in the transient (needs a forcing that does not scale with load).
4. Rails, ducts, wires as mass; device capacity from part data (default 500 J/K per dm2 dominates).
5. Plate emissivity and wall radiation from H2 once merged.
6. Validation with H6 against a measured heat-up.
