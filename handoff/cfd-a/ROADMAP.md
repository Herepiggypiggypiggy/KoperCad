# CFD-A flow core: roadmap

Design: docs/design/thermal-cfd.md. Crate: crates/novo-cfd (owner: this lane for the core; turb/, mesh/, sched/, solids/, openings/, rad/ belong to lanes B, C1, E, C2, D).

1. PR 1 (done in this PR): result contract (Grid3, Fidelity, Progress, CfdSnapshot, MaterialInfo, AccuracyStatement), Solver trait, Control (cancel flag), StubSolver.
2. 3D staggered MAC grid, fractional-step projection, implicit diffusion.
3. TVD/QUICK convection, Boussinesq buoyancy, energy equation.
4. Multigrid-preconditioned CG pressure.
5. Benchmarks with error numbers: lid cavity Re 100/1000 (Ghia), differentially heated cavity Ra 1e3-1e6 (de Vahl Davis), heated vertical plate (Churchill-Chu).
