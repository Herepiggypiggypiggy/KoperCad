# CFD-D radiation: roadmap

Scope: grey surface-to-surface radiation in `crates/novo-cfd/src/rad/` (design `docs/design/thermal-cfd.md`, section 2).

Done (PR 1, engine)
- Patch set from the CFD-C1 `Mesh`: exposed solid faces and enclosure wall faces, grouped into at most `max_patches` flat patches.
- View factors by deterministic ray sampling on the fine grid (solids shadow), exact reciprocity and summation by symmetric scaling.
- Radiosity solve, net flux per patch, linearised coefficient for implicit coupling, power per material.
- `HeatPaths` (convection / conduction / radiation split).
- Tests against analytic view factors, direct dense solve, two-surface formula, shadowing, energy balance.

Next
1. Coupling with the solid conduction (needs CFD-C2 solid faces on main): `face_temperatures -> RadiationModel::solve -> scatter` as a source at solid face cells, under-relaxed, `h_lin` implicit part. Interface agreed with the CFD-C2 thread.
2. Wall temperature from the outer film (IEC 60890 coefficient, CFD-C2) as the wall patch temperature.
3. Heat split per device in the result: `HeatPaths` per device in `CfdSnapshot` (needs CFD-A to add the field) and in the Simulate result card (CFD-F).
4. Toggle in the Simulate dialog (CFD-F reads `RadSettings::enabled`).
5. Emissivity from part data (`novo-parts` key, ask PARTS lane) instead of planning defaults.
6. Validation entries for CFD-G (concentric, parallel plate exchange).

Open questions
- Where `HeatPaths` lives in the result (CfdSnapshot field vs side struct).
