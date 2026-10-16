# CFD-A flow core: progress

- 2026-10-16: skeleton crate with contract, Solver trait, StubSolver, 5 contract tests; fmt/clippy/test clean locally.
- 2026-10-16: flow core (src/flow): staggered MAC grid on non-uniform Cartesian meshes, fractional step, explicit TVD (van Leer) / QUICK convection, implicit diffusion, Boussinesq buoyancy, energy equation, aggregation-multigrid-preconditioned CG pressure. Laminar, closed walls only.
  Error numbers (tests print them with --nocapture):
  - Poisson, manufactured cos solution: max error 3.2e-3 (16^3), 8.0e-4 (32^3), second order; 21-30 CG iterations.
  - Lid cavity vs Ghia (15 points each, errors in units of lid speed): Re 100 48x48 u max 0.0050, v max 0.0092; Re 1000 48x48 stretched u max 0.0102, v max 0.0067.
  - Differentially heated cavity vs de Vahl Davis, Nu / u_max / v_max error: Ra 1e3 (48^2) +0.55/+0.16/+2.97 %; 1e4 (32^2) +0.62/+0.68/+0.99 %; 1e5 (40^2) +0.15/+3.06/+0.94 %; 1e6 (56^2) +0.29/+2.19/+1.50 %. Ra 1e3 v_max converges slowly (4.5 % at 32, 3.0 % at 48, 1.7 % at 96).
  - Conduction between fixed walls: max error 8.5e-10 K. Closed adiabatic box energy: exact to 1e-8 relative. Still air stays still (1e-16 m/s).
  - Test time about 160 s total (Re 1000 cavity 60 s, heated cavity 90 s, run in parallel).
- Design choice: Boussinesq reference is the volume-mean temperature (closed box: a uniform offset is a pressure gradient; removes a splitting error of 3.6e-4 m/s). Openings lane C2 must revisit.

- 2026-10-16: #575 merged (R1 READY). New: `ThermalBc::Convective { h, t_out }` and `FlowConfig::conductivity` (half air cell in series with the film; the outside wall of the IEC 60890 film belongs here); test against the series-resistance solution, error 1e-9 K.
- Heated vertical plate vs Churchill-Chu: tried as the hot wall of a closed square cavity against the local similarity formula; deviates +38 % mean at Ra 1e6 because the layer is thick (about 0.1 H) and the core is stratified, so the comparison is not well posed. Not shipped as a test. Needs an open (inflow/outflow) boundary, which comes with lane C2 openings; then do it as a free plate, and at Ra above 1e8 for the thin-layer regime.
- Accuracy statement input (R1 note): time stepping is first order and convection explicit, so the steady state is exact in the limit but transients are only first-order accurate; CFL 0.4 limits the step.
- 2026-10-16: turbulence hooks in flow/solver.rs for lane CFD-B: `FlowSolver::{nu_t, wall_nu, wall_alpha}` and `FlowConfig::prandtl_t`. Momentum diffuses with nu + nu_t (harmonic mean per face), heat with k + rho cp nu_t / Pr_t; wall tables replace nu / alpha between a cell centre and a domain wall or a solid (series with the solid half cell). None = bit-identical laminar (tested). Neglected: stress-tensor cross terms with grad nu_t.
