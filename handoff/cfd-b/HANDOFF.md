# CFD-B turbulence: handoff

Branch `claude/cfd-b-turbulence-9reum0` (repo kopercad), always restarted from main after a merge. Engine `crates/novo-cfd/src/turb/`: mesh.rs (wall distance, wall faces), wall.rs (Spalding, Kader), transport.rs (implicit upwind scalar solver), mod.rs (SST, k-epsilon), coupling.rs (`TurbulentFlow`: sets `flow.nu_t`, `flow.wall_nu`, `flow.wall_alpha` each step, then advances the turbulence). Tests: turb_basics, turb_channel, turb_coupling (fast); turb_plate (slow, `--ignored`).

Validated (numbers printed by the tests):
- Channel Re_tau 590: SST resolved 4.7 % log-layer error, centreline U+ 20.69 vs DNS 21.3; SST wall function 2.7 %; k-epsilon 7.1 %; the two agree within 1 %.
- Heated wall vs Churchill-Chu (Nu of wall-to-core temperature difference): SST resolved +4.3 % (Ra 1.0e9, 40x80, 16 min) and +9.8 % (Ra 3.4e9, 28x56); k-epsilon on the resolved grid +34.5 % (not made for resolved buoyant layers); coarse wall cells 16x48: SST and k-epsilon agree (85.6 vs 85.0) but are 52 % low. State this in the accuracy text: wall functions are not valid for natural-convection boundary layers thinner than the first cell.
- Hot/cold wall flux mismatch in the plate runs is 5 %.

Cabinet solver (PR 4): `CabinetSolver::with_turbulence(TurbModel)` (default Laminar, nothing changes). `MarchState.turb` is pushed to the flow before each step and advanced after it (3 sweeps); every snapshot gets `turb_visc_ratio` and `accuracy_with_turbulence` text. Since #688 the default steady path (`solve_steady`) also runs the turbulence (laminar and SST); k-epsilon and the marched method (`Tuning::marched()`) use `MarchState.turb`. The accuracy text names the model only where it ran. Example: `cargo run --release -p novo-cfd --example turb_vented -- sst 6000` (also `laminar`, `ke`).

Vented 600x800x300 box, 100 W, two 200x120 louvres, vs the 2D engine (mean air rise 6.93 K, device surface rise 99.5 K). Mean air rise (and device surface rise) laminar / SST / k-eps, main after #688/#708 (steady path for laminar and SST, march for k-eps), with the fixes below:
- 1792 cells: 5.88 (-15.2 %) / 5.92 (-14.6 %) / 5.95 (-14.2 %); surface +4.6 / +4.4 / +4.4 %
- 6000 cells: 5.62 (-18.9 %) / 5.53 (-20.2 %) / 5.80 (-16.3 %); surface +9.8 / +9.6 / +9.3 %
Turbulence does NOT close the gap and changes the answer by 1 to 3 K-percent: eddy viscosity is small (nu_t/nu max 40 to 110 locally) and the device sits behind CFD-A's film model. The table in the #694 handoff (SST closing a quarter of the gap) was measured before the device film model existed and is superseded. Top louvre exhaust rise (3402 cells): 2D 8.99 K, 3D laminar 12.98, SST 13.64, k-eps 14.39 K. Cause of the remaining gap: unknown. Candidates: the 2D vent heat model (the 3D vents carry 12 to 19 % more heat than 2D, see #681), 3D wall loss. Not radiation and not the plate share. CFD-C2: wall-weighted boundary rise within 1 % of 2D, the volume mean is lower because cool inlet air fills the lower box. Runtime SST 1.3x laminar (steady).

Second-order convection for k and omega/epsilon (`TurbConfig.second_order`, `CabinetSolver::with_second_order_turbulence`): van Leer limited deferred correction on the implicit upwind step. Step test: transition width 16 to 10 cells, no over/undershoot. Heated wall benchmark: SST +9.8 % to +9.6 %, coarse wall cells unchanged, vented box unchanged: the limits are the wall treatment, not the convection of k and omega. Default stays first-order.

Traps found with the cabinet solver (all in `tests/cabinet.rs`, the turbulence test fails without them):
- The flow core scaled every interior solid-air conductance by `eff/mol` when an eddy viscosity is set, including the device film faces (`k_face` overridden by the film model, which already is the whole wall layer): the device cooled by 35 %. `film_mask` in the flow core skips them.
- `wall_alpha` replaced the film conductance on device surfaces and on cells with any solid share (sheet-metal wall inside a cell: blended heat capacity and conductivity): device 4x too hot. `TurbState` gives no wall alpha to device neighbours (`film`) or to cut cells (`cut`).

Cut cells and device surfaces fall back to the flow core's own conductance (see traps above). Those faces fall back to molecular alpha plus the eddy part of k_eff, so the turbulent heat transfer at a cut device surface is under-resolved (conservative: never too much).

Approximations at openings (first cut): a vent covers only its free-area share of the face, the flow core still treats the closed share as wall (`closed = 1 - open share`) while the turbulence side drops the wall for the whole boundary face of the cell; zero-gradient symmetry lets the inflow carry the cell's own k/omega instead of ambient-level turbulence. Both small at 100 W / 0.1 m/s.

Traps
- Start the wall layer from the log-law guess (`new` does it); a uniform start falls on a laminar-like branch.
- Wall-cell strain is overridden by u_tau^2/(nu + kappa u_tau d).
- `TurbulentFlow::new` reads the solid mask from `flow.cfg.medium` (fluid_frac < `solids::MIN_OPEN`); partly-open cells count as fluid.
- `StepInfo` of the flow core is not exported, so `TurbulentFlow::step` returns `(max_accel, max_dtdt)`.
- The wall functions use the absolute tangential speed (moving walls not subtracted yet).
- Plate test with the cavity uses the core temperature at mid height; it is a cavity, not an infinite plate.

Contacts: R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, CFD-A `session_01ES8fwPNsJAC5x14QbAkDQ9`.
