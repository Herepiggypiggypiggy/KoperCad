# Hoist families handoff

Branch claude/project-thread-8de6e0 (kopercad). Motor/drive thread owns the shared hoist model; it agreed not to touch hoist_unit.rs, brake.rs, spec.rs for a day.
Resume: `cargo test -p novo-drive -p novo-machine`; tests in crates/novo-drive/tests/hoist_families.rs and crates/novo-machine/tests/hoist_families_loop.rs.
User finds it: Simulate > Hoist run, motor property sheet "Mechanical load" (mech_brake_kind = conical, mech_fine_lift_motor = tag of second motor).
Decisions: a phase open in the drawing is taken as L3; fine-lift is a rigid ratio while fed, free while not; thresholds and delays are user numbers.

Second PR (branch claude/project-thread-8de6e0 from main 982c3fc): current, heating and overload relay. Property sheet keys mech_voltage_v, mech_*_amps/_pf, mech_rated_rise_k, mech_rise_limit_k, mech_overload_amps/_class_s/_contact. Rated current empty = derived from power with a generic efficiency.
