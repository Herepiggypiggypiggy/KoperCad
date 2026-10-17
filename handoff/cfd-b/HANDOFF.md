# CFD-B turbulence: handoff

Branch `claude/cfd-b-turbulence-9reum0` (repo kopercad), always restarted from main after a merge. Engine `crates/novo-cfd/src/turb/`: mesh.rs (wall distance, wall faces), wall.rs (Spalding, Kader), transport.rs (implicit upwind scalar solver), mod.rs (SST, k-epsilon), coupling.rs (`TurbulentFlow`: sets `flow.nu_t`, `flow.wall_nu`, `flow.wall_alpha` each step, then advances the turbulence). Tests: turb_basics, turb_channel, turb_coupling (fast); turb_plate (slow, `--ignored`).

Validated (numbers printed by the tests):
- Channel Re_tau 590: SST resolved 4.7 % log-layer error, centreline U+ 20.69 vs DNS 21.3; SST wall function 2.7 %; k-epsilon 7.1 %; the two agree within 1 %.
- Heated wall vs Churchill-Chu (Nu of wall-to-core temperature difference): SST resolved +4.3 % (Ra 1.0e9, 40x80, 16 min) and +9.8 % (Ra 3.4e9, 28x56); k-epsilon on the resolved grid +34.5 % (not made for resolved buoyant layers); coarse wall cells 16x48: SST and k-epsilon agree (85.6 vs 85.0) but are 52 % low. State this in the accuracy text: wall functions are not valid for natural-convection boundary layers thinner than the first cell.
- Hot/cold wall flux mismatch in the plate runs is 5 %.

Cabinet solver (PR 4): `CabinetSolver::with_turbulence(TurbModel)` (default Laminar, nothing changes). `MarchState.turb` is pushed to the flow before each step and advanced after it (3 sweeps); every snapshot gets `turb_visc_ratio` and `accuracy_with_turbulence` text. Since #688 the default steady path (`solve_steady`) also runs the turbulence (laminar and SST); k-epsilon and the marched method (`Tuning::marched()`) use `MarchState.turb`. The accuracy text names the model only where it ran. Example: `cargo run --release -p novo-cfd --example turb_vented -- sst 6000` (also `laminar`, `ke`).

Vented 600x800x300 box, 100 W, two 200x120 louvres, vs the 2D engine (air rise 6.93 K, device surface rise 99.5 K). Mean air rise, laminar / SST / k-eps, and device surface rise:
- 1890 cells: 6.11 (-11.9 %) / 6.13 (-11.5 %) / 6.19 (-10.7 %); surface +1.4 / -2.7 / -3.2 %
- 6240 cells: 5.69 (-18.0 %) / 6.00 (-13.5 %) / 6.04 (-12.9 %); surface +7.1 / +1.9 / +0.9 %
- 12064 cells: 5.59 (-19.4 %) / 5.90 (-15.0 %) / 6.09 (-12.2 %); surface +9.1 / +3.2 / +1.8 %
Turbulence does NOT close the gap in the mean air rise: SST removes about a quarter of it on the fine meshes, k-epsilon about a third, and both stop the laminar drift with mesh refinement (laminar surface rise grows from +1.4 to +9.1 %). The eddy viscosity is modest (nu_t/nu max 17 SST, 99 k-eps). Cause of the remaining 12-15 %: unknown. Candidates: the 2D vent heat model (the 3D vents carry 12 to 19 % more heat than 2D, see #681), 3D wall loss. Not radiation (missing radiation would make 3D hotter, not cooler) and not the plate share. CFD-C2 found the wall-weighted boundary rise within 1 % of 2D (6.99 vs 6.93 K): the volume mean is lower because cool inlet air fills the lower box, so the volume mean is a poor metric. Exhaust through the top louvre at 3402 cells (`EXHAUST` line of the example), rise laminar / SST / k-eps 13.50 / 13.24 / 12.75 K vs 2D 8.99 K: turbulent mixing lowers it by only 0.3 to 0.75 K. Runtime 1.4 to 2x laminar.

Trap: never give `wall_alpha` to cells that hold part solid (fluid_frac < 1): the flow core puts it in series with `alpha * c_cell` and the blended heat capacity made the device 4x hotter. `TurbState::push_to_flow` skips them. Those faces fall back to molecular alpha plus the eddy part of k_eff, so the turbulent heat transfer at a cut device surface is under-resolved (conservative: never too much).

Approximations at openings (first cut): a vent covers only its free-area share of the face, the flow core still treats the closed share as wall (`closed = 1 - open share`) while the turbulence side drops the wall for the whole boundary face of the cell; zero-gradient symmetry lets the inflow carry the cell's own k/omega instead of ambient-level turbulence. Both small at 100 W / 0.1 m/s.

Traps
- Start the wall layer from the log-law guess (`new` does it); a uniform start falls on a laminar-like branch.
- Wall-cell strain is overridden by u_tau^2/(nu + kappa u_tau d).
- `TurbulentFlow::new` reads the solid mask from `flow.cfg.medium` (fluid_frac < `solids::MIN_OPEN`); partly-open cells count as fluid.
- `StepInfo` of the flow core is not exported, so `TurbulentFlow::step` returns `(max_accel, max_dtdt)`.
- The wall functions use the absolute tangential speed (moving walls not subtracted yet).
- Plate test with the cavity uses the core temperature at mid height; it is a cavity, not an infinite plate.

Contacts: R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, CFD-A `session_01ES8fwPNsJAC5x14QbAkDQ9`.
