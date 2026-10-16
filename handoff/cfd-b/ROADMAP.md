# CFD-B turbulence: roadmap

Scope: k-omega SST (default) and k-epsilon cross check with wall functions in `crates/novo-cfd/src/turb/`; effective viscosity and conductivity for the flow core; `turb_visc_ratio` in `CfdSnapshot`. Laminar benchmarks must not change.

Done
- PR 1: engine on the cell grid (SST, k-epsilon, Spalding and Kader wall functions, wall distance, buoyancy production), channel validation.

Next, in order
1. Hooks in the flow core (CFD-A owns it): per-cell `nu_eff`, `alpha_eff`, wall shear and wall heat flux from `TurbState::walls()`. Then `Solver` trait glue: `TurbState::advance` once per flow step.
2. Heated vertical plate, Ra > 1e9, against Churchill-Chu; SST vs k-epsilon agreement in a natural-convection cavity.
3. Second-order convection for k and omega (TVD), turbulence at solid cells from lane C2 (solid mask is already an input).
4. Wall function on openings and fans once lane C2 lands.

Open questions
- Where CFD-F exposes the model choice (default SST, k-epsilon as "cross check").
