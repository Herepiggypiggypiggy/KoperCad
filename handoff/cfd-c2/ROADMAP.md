# CFD-C2 solids and openings: roadmap

Scope (brief: /mnt/project-files/plans/spawn-table.md, CFD-C2): conjugate heat transfer in the flow grid, and openings. Owns `crates/novo-cfd/src/solids/` and `src/openings/`; touches `src/flow/{solver,pressure}.rs` additively (open faces, solid faces) with CFD-A informed.

1. PR 1 (this one): open boundaries and openings. Free, vent (orifice K = 1/(Cd phi)^2, the novo-thermal law) and fan (maker line as resistance plus pressure jump). Pressure Poisson with conductance to ambient. Buoyancy reference = outside air. Tests: patch areas, fan+vent operating point, exhaust fan + free inlet, stack effect, still ambient. Unblocks Churchill-Chu free plate (CFD-G).
2. PR 2: solids. Face open fractions (blocked faces, wall friction), per-cell rho cp, harmonic-mean face conductivity, volumetric sources, wall layer with IEC 60890 outside film. Tests: 1D layered conduction (analytic), energy balance below 0.1 percent.
3. PR 3: slotted ducts as anisotropic porous media (Darcy-Forchheimer), device and rail material data replacing the planning values in `mesh/regions.rs` (ask CFD-C1), scenario-to-FlowConfig driver pieces if nobody else owns them.
4. Later: fan swirl/inclined fans, fans inside the volume as momentum source regions, time-dependent fan on/off.

Open questions: who writes the scenario to FlowSolver driver (mesh classification to solver); asked lead A.
