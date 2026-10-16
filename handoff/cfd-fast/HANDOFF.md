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
- Steady with the same fan: mass balances to 2e-6 (patch flows +0.02346 / -0.02346 m3/s), first-order stage closes the heat to +0.3%, the second-order (VanLeer deferred) stage ends at -2.8% (out 51.4 W for 50 W) with a wobbling jet, window test accepted it. Not proven whether this is a slow drift the noise-aware tolerance lets through or a real leak of the deferral next to open faces. Next step: run the second stage longer and with a tighter tol_balance on this case; compare the series imbalance against the stored-heat drift.
- Test advice for #707: steady heat balance within 3.5% in the fan case; compare with the march only where the march's own balance is within 3%.
