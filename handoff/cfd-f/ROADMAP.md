# CFD-F results and app: roadmap

Done:
1. CfdSnapshot helpers in novo-heatview (slice, as_2d, streamlines, iso_surface); Simulate on a worker; fidelity badge, cancel, out-of-date mark, per-device margin on badges (#580, #596).
2. A/B compare: Δ, Wipe and Side (#625, #633, #641).
3. Simulate through the novo-cfd scheduler (Coarse, Medium, Fine), progress bar and residual sparkline (#652). `heat_run::default_solver` now runs `CabinetSolver` (CFD-INT).
4. 3D face: Heat strip in the 3D workspace with slice plane, temperature shell, streamlines and animated tracers on the View3d overlay API (#664, #666).

Open:
- Time slider for transient results: needs transient frames in the result contract (CFD-A); the solvers publish steady states only.
- Cross-fade between fidelity levels and an ETA in the progress bar.
- Colour legend inside the 3D view (the renderer thread offered to add one).
