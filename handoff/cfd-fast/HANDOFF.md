# CFD-FAST handoff

State: PR #688 adds `FlowSolver` steady path (module `flow/solver/steady/`): pseudo-transient, implicit first-order upwind convection with deferred correction to the configured scheme, incremental (SIMPLEC-like) pressure correction, implicit energy with a longer step, window-mean convergence (second-order plumes wobble and never reach a fixed point). The old `step()` march is untouched and still selectable.

Use: `SteadyRun::staged(&mut solver, SteadyConfig::first_order(), SteadyConfig::second_order(), SteadyLimits::default())`, `advance`, `report`. `initialise_from(&coarse)` for grid sequencing. Probes: `examples/steady_probe.rs`, `examples/steady_cabinet.rs` (`<cells> steady|march [cfl] [threads]`, env knobs in the file header).

Open:
- CFD-INT: select steady path in the cabinet march; keep old march selectable. `tests/cabinet.rs::vents_carry_heat_out_and_cool_the_device` fails on main independent of this work.
- Mirror CFD-A heat_scale/radiation source in `assemble_energy` (steady/assemble.rs) when #686 lands.
- Speed: momentum assembly (about 30% of an iteration) is generic and allocation heavy; pressure solve scales poorly past 2 threads; sweeps are serial (bit-identical choice).
- Not validated: Churchill-Chu plate and IEC 60890 sealed box on the steady path (the IEC gap belongs to CFD-C2). Open-face law only tested on small vent/fan cabinets.
- Stability: pseudo step is limited by buoyancy frequency (buoyancy_cfl 1; 2 diverges).

Fan + porous duct (tests/flow_steady.rs): steady closes the heat balance to 0.1 %, the march leaks about 3 % with a fan (51.7 W out for 50 W in), the device is +0.9 K above the march (8 % of the rise), fan flow agrees to 0.1 %. Cause not found; fan+duct steady is not validated better than ~8 %.
Turbulence: LevelTuning.turbulent (and, after #694, CabinetSolver.turbulence != Laminar) forces the march; wiring TurbState into steady_iterate is a follow-up.
heat_scale is applied in steady energy and heating_w; the radiation source extra_heat_w_m3 (CFD-A) is not mirrored yet.

## Open defects (2026-10-16, found on #707's refined mesh, fan cabinet 1890 cells)
- Reference march with a fan creates energy: 57.07 W out for 50 W in (14%; 3% on the old mesh). Suspect: open-face energy treatment in FlowSolver::step (inflow/outflow at cells whose divergence is not zero after the projection). Not investigated further; the march is the reference path, not mine.
- Steady with the same fan: mass balances to 2e-6, first-order stage +0.3%, second-order stage ends -2.8% with the default window (100). It is window-mean noise of the wobbling jet, not a leak: window 400 gives -0.91% (5600 it), second stage only from scratch gives +1.7% (700 it); sign changes, Tmax 55.18 / 55.32 / 55.26 C. Treat the heat balance of a fan case as +-3% on a mean over one window; longer windows tighten it.
- Test advice for #707: steady heat balance within 3.5% in the fan case; compare with the march only where the march's own balance is within 3%.

## 3D heat result tools (2026-10-16)
Heat strip in the 3D view (after Cabinet > Heat > Simulate): Slice (axis, mm field, cell steps), Shell, Airflow, Arrows, Devices, Probe, colour key, Hot spots menu. Code: `crates/novo-app/src/app/heat_3d.rs` (overlays), `heat_probe.rs`, `heat_legend.rs`, `heat_spots.rs`, `heat_arrows.rs`; strip and wiring in `view3d.rs` (`heat_controls`). Read points through `novo_heatview::cfd` samplers. All read the model only; none validates the solver.
