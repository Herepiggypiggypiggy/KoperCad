# Hoist families handoff

Branch claude/project-thread-8de6e0 (kopercad). Motor/drive thread owns the shared hoist model; it agreed not to touch hoist_unit.rs, brake.rs, spec.rs for a day.
Resume: `cargo test -p novo-drive -p novo-machine`; tests in crates/novo-drive/tests/hoist_families.rs and crates/novo-machine/tests/hoist_families_loop.rs.
User finds it: Simulate > Hoist run, motor property sheet "Mechanical load" (mech_brake_kind = conical, mech_fine_lift_motor = tag of second motor).
Decisions: a phase open in the drawing is taken as L3; fine-lift is a rigid ratio while fed, free while not; thresholds and delays are user numbers.

Second PR (branch claude/project-thread-8de6e0 from main 982c3fc): current, heating and overload relay. Property sheet keys mech_voltage_v, mech_*_amps/_pf, mech_rated_rise_k, mech_rise_limit_k, mech_overload_amps/_class_s/_contact. Rated current empty = derived from power with a generic efficiency.

Third PR (branch claude/hoist-3d-hook from main 41db81b): the hook in 3D.
- novo-view3d/src/crane.rs: pure scene builder (floor, runway beams, girder on end trucks, trolley, drum, rope, hook block, load, limit flags) from `CraneLayout` and `CraneState`; `block_top_mm` clamps the block at the drum and the floor and reports it; `hook_bounds` frames the hook.
- novo-app/src/app/hoist_3d.rs: window "Hoist in 3D" (command `sim.hoist3d`, Simulate menu, Ctrl+K). Takes hook height, warnings and limit heights from the Hoist run loop (`HoistRun::module_at`); bridge, trolley and the crane size are typed in.
- Not bound: bridge and trolley do not move from the travel drives (TravelLoop has position_m but its spec does not say bridge or trolley); no rope sway; no per-crane sizes from the project.
- R2 follow-ups folded in: rated current from power says it uses an assumed efficiency; the relay line says it protects the main motor only.
