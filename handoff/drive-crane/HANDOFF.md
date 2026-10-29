# Drive and crane: handoff

Crates `novo-drive` and `novo-machine` are on main (L0). L2a adds the two-speed hoist engine in `novo-drive` (`motor`, `sequencer`, `brake`, `mechanics`, `hoist_unit`, `error`).
Entry point for the loop lane (L5a): `hoist_unit::TwoSpeedHoistUnit::step(HoistInput, dt)` returns a `HoistSnapshot`; the schematic supplies `Command` (direction, step 0/1/2), `mains_on` and, with `BrakeFeed::Separate`, the brake coil.
Brake torque of a tick is the value at its start (one tick of latency, deterministic).
Conventions: positive shaft speed raises the hook; see `mechanics.rs` header for the efficiency convention.
Resume: `cargo test -p novo-drive -p novo-machine`.
No UI entry yet: the "Mechanical load" section of the motor property sheet (L3/L6) and the loop (L5a) consume it.

L7 validation: `docs/design/drive-crane-validation.md` lists every case, what runs now and which lane un-ignores which test. A lane that lands engine code replaces the `todo!` in its tests and keeps the hand value; disagreements go to the doc, not to a changed hand value.

L2b (motor at variable frequency): `novo_drive::induction`. Entry points for the travel drive loop: `InductionMotor::from_nameplate` (or `InductionMotor::new` from circuit values), `Supply::new(freq, volts_ll, direction)` from `DriveOutputs`, then `step_shaft(&mut omega, &supply, inertia, dt, load_fn)` returns `ShaftStep` with `point.stator_current_a` and `point.input_power_w` for `DriveInputs::motor_current_a/motor_power_w`. `frequency_row` gives the capability at a frequency under a `VfCurve` and current limit.
Conventions: reactances are given at the rated frequency per phase of the equivalent star (a delta winding impedance is divided by 3); torque and speed in the shaft sense, slip in the field sense; input power is negative when the motor feeds back.
Fit: Rs = rs_over_rr x Rr and equal leakage split by default; when the nameplate losses exceed copper plus the entered core and friction, the surplus goes to core and friction and a breakdown ratio is assumed (reported in `FitReport::assumed_breakdown_ratio` and a note). Constant rotor resistance underestimates standstill torque of deep-bar motors.
Not modelled: flux transients, harmonics, skin effect, temperature. Torque follows the steady-state curve instantly.
Resume: `cargo test -p novo-drive`. UI: Simulate > Motor at variable frequency.
