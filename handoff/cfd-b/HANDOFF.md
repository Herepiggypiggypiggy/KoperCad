# CFD-B turbulence: handoff

Branch `claude/cfd-b-turbulence-9reum0` (repo kopercad), always restarted from main after a merge. Engine `crates/novo-cfd/src/turb/`: mesh.rs (wall distance, wall faces), wall.rs (Spalding, Kader), transport.rs (implicit upwind scalar solver), mod.rs (SST, k-epsilon), coupling.rs (`TurbulentFlow`: sets `flow.nu_t`, `flow.wall_nu`, `flow.wall_alpha` each step, then advances the turbulence). Tests: turb_basics, turb_channel, turb_coupling (fast); turb_plate (slow, `--ignored`).

Validated (numbers printed by the tests):
- Channel Re_tau 590: SST resolved 4.7 % log-layer error, centreline U+ 20.69 vs DNS 21.3; SST wall function 2.7 %; k-epsilon 7.1 %; the two agree within 1 %.
- Heated wall vs Churchill-Chu (Nu of wall-to-core temperature difference): SST resolved +4.3 % (Ra 1.0e9, 40x80, 16 min) and +9.8 % (Ra 3.4e9, 28x56); k-epsilon on the resolved grid +34.5 % (not made for resolved buoyant layers); coarse wall cells 16x48: SST and k-epsilon agree (85.6 vs 85.0) but are 52 % low. State this in the accuracy text: wall functions are not valid for natural-convection boundary layers thinner than the first cell.
- Hot/cold wall flux mismatch in the plate runs is 5 %.

Cabinet solver (PR 4): `CabinetSolver::with_turbulence(TurbModel)` (default Laminar, nothing changes). `MarchState.turb` is pushed to the flow before each step and advanced after it (3 sweeps); every snapshot gets `turb_visc_ratio` and `accuracy_with_turbulence` text. Example: `cargo run --release -p novo-cfd --example turb_vented -- sst 6000` (also `laminar`, `ke`).

Vented 600x800x300 box, 100 W, two 200x120 louvres, vs the 2D engine (air rise 6.93 K, device surface rise 99.5 K). Mean air rise, laminar / SST / k-eps, and device surface rise:
- 1890 cells: 6.11 (-11.9 %) / 6.13 (-11.5 %) / 6.19 (-10.7 %); surface +1.4 / -2.7 / -3.2 %
- 6240 cells: 5.69 (-18.0 %) / 6.00 (-13.5 %) / 6.04 (-12.9 %); surface +7.1 / +1.9 / +0.9 %
- 12064 cells: 5.59 (-19.4 %) / 5.90 (-15.0 %) / 6.09 (-12.2 %); surface +9.1 / +3.2 / +1.8 %
Turbulence does NOT close the gap in the mean air rise: it removes about a quarter of it on the fine meshes and stops the laminar drift with mesh refinement (laminar surface rise grows 1.4 to 9.1 %). The eddy viscosity is modest (nu_t/nu max 17 SST, 99 k-eps). Mean air rise follows from energy balance minus wall losses, so the remaining 12-15 % is more likely wall heat loss in 3D (side walls, radiation not modelled) than flow; check with CFD-C2. Runtime 1.4 to 2x laminar.

Trap: never give `wall_alpha` to cells that hold part solid (fluid_frac < 1): the flow core puts it in series with `alpha * c_cell` and the blended heat capacity made the device 4x hotter. `TurbState::push_to_flow` skips them.

Traps
- Start the wall layer from the log-law guess (`new` does it); a uniform start falls on a laminar-like branch.
- Wall-cell strain is overridden by u_tau^2/(nu + kappa u_tau d).
- `TurbulentFlow::new` reads the solid mask from `flow.cfg.medium` (fluid_frac < `solids::MIN_OPEN`); partly-open cells count as fluid.
- `StepInfo` of the flow core is not exported, so `TurbulentFlow::step` returns `(max_accel, max_dtdt)`.
- The wall functions use the absolute tangential speed (moving walls not subtracted yet).
- Plate test with the cavity uses the core temperature at mid height; it is a cavity, not an infinite plate.

Contacts: R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, CFD-A `session_01ES8fwPNsJAC5x14QbAkDQ9`.
