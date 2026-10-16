# CFD-B turbulence: roadmap

Scope: k-omega SST (default) and k-epsilon cross check with wall functions in `crates/novo-cfd/src/turb/`; effective viscosity and conductivity for the flow core; `turb_visc_ratio` in `CfdSnapshot`. Laminar benchmarks must not change.

Done
- PR 1 (#578, merged): engine on the cell grid, wall functions, channel validation.
- PR 2 (#669, merged): `TurbulentFlow` coupling through the #650 hooks, laminar bit-identical test, heated-wall benchmark vs Churchill-Chu (slow tests, `--ignored`).

- PR 3: solid cells of `cfg.medium` (fluid share below MIN_OPEN) are walls for the turbulence; zero-alpha guard.

- PR 4: turbulence option in CabinetSolver, snapshot ratio, accuracy text, vented box study.

Next, in order
1. Openings (inflow/outflow turbulence values) once the open faces are stable in lane C2.
2. Model choice in the UI (CFD-F); effect of solid walls on a cabinet temperature is still unmeasured against a reference.
3. Second-order convection for k and omega; moving-wall velocity relative to the wall in the wall functions.
4. A wall-function variant for buoyant boundary layers on coarse cells (coarse cells are 50 % low today); or mesh rule from lane C1: resolve the wall layer on hot surfaces.
5. Hand the numbers to CFD-G for the accuracy statement.

Open questions
- Where CFD-F exposes the model choice (default SST, k-epsilon as "cross check").
