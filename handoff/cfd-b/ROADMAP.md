# CFD-B turbulence: roadmap

Scope: k-omega SST (default) and k-epsilon cross check with wall functions in `crates/novo-cfd/src/turb/`; effective viscosity and conductivity for the flow core; `turb_visc_ratio` in `CfdSnapshot`. Laminar benchmarks must not change.

Done
- PR 1 (#578, merged): engine on the cell grid, wall functions, channel validation.
- PR 2: `TurbulentFlow` coupling through the #650 hooks, laminar bit-identical test, heated-wall benchmark vs Churchill-Chu (slow tests, `--ignored`).

Next, in order
1. Solid cells and openings: pass the solid mask from lane C2/C1 into `TurbState::new` (supported already), wall values on solid faces (hooks exist).
2. Hook `TurbulentFlow` into the `Solver` trait glue / scheduler (lane E, CFD-A) and `turb_visc_ratio` into the snapshot; model choice option in CFD-F.
3. Second-order convection for k and omega; moving-wall velocity relative to the wall in the wall functions.
4. A wall-function variant for buoyant boundary layers on coarse cells (coarse cells are 50 % low today); or mesh rule from lane C1: resolve the wall layer on hot surfaces.
5. Hand the numbers to CFD-G for the accuracy statement.

Open questions
- Where CFD-F exposes the model choice (default SST, k-epsilon as "cross check").
