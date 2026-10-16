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
