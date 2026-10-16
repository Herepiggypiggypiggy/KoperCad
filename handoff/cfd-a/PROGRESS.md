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
- Next: heated vertical plate vs Churchill-Chu; implement `Solver` for the flow core (Fidelity::Coarse from a Scenario) once lane C1 meshing lands; rayon kernels are lane E.
