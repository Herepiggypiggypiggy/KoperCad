# CFD-C2: progress

- 2026-10-16 08:36 UTC thread started from the spawn table; main at 15a39e2 (flow core, turbulence, scheduler, meshing, film wall condition).
- 2026-10-16: open boundaries in the flow core: `openings/` (patches, law), `PressureSolver::set_open_conductance`, `FlowConfig::{openings,t_ref,rho}`, open-face prediction/finish in `FlowSolver::project`, transverse momentum and enthalpy flux through open faces.
- 2026-10-16: tests in tests/openings.rs: fan+vent operating point error -1.2 %, exhaust fan with free inlet exact to 0.01 %, still ambient 6e-14 m/s, stack effect Q within -9 % of the ideal stack law (energy 0.07 %, 8x power gives 2x flow). All earlier novo-cfd tests unchanged.
