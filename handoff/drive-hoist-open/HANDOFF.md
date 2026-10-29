# Open-loop drive hoist: handoff

Branch `claude/project-thread-f0c2p3` (kopercad). One PR from main.
Entry points: `novo_drive::hoist_logic::{HoistLogic, HoistLogicConfig}` (pure state machine),
`novo_machine::drive_hoist_loop::{DriveHoistLoop, DriveHoistModule, DriveHoistParts}`,
`novo_machine::drive_hoist_spec::drive_hoist_from_properties` (`dh_*` keys, marker `dh_mass_kg`).
UI: `crates/novo-app/src/app/drive_run.rs` (`drive_hoists_of`, `hoist_reading`), property section in
`inspector.rs` ("Drive hoist"). Window: Simulate > Drive run.

How it ticks: limits and the drive's brake relay contact are written to the circuit, the circuit settles,
the logic reads up/down/speed inputs and the previous tick's drive output and motor, `Drive::step` ramps
under the reference (word reference, inputs forward/reverse/fault reset), the brake follows its coil
(circuit feed or the relay directly), flux builds with the rotor time constant, the shaft advances with
stiction against friction plus brake torque. Positive speed raises the hook.

Decisions and traps
- The logic opens the brake on motor current, not on torque; the model's standstill torque at 1 to 3 Hz
  depends strongly on the V/f boost, so the tests use a 20 V boost (a 30 V boost over-excites a model
  without saturation and holds anything).
- A sinking hook that never recovers is reported at 50 mm (`RUNAWAY_M`), not at settle.
- A maker's function names are not reproduced: "load sensing" is generic.
- Test numbers are worked from a 5.5 kW motor, 2000 kg, 63:1, 2 falls (23.3 N m on the shaft, 64 % of rated).

QA: scenarios `1709-drive-hoist-run` and `1709b-drive-hoist-run-1920` use `qa/fixtures/hoist-drive.novo` (hold S2: sensed load finding, brake closes on release).
Resume: `cargo test -p novo-drive -p novo-machine`, then `cargo test -p novo-app --lib drive_run`.

Update: jump at reversal is built (blank `dh_jump_hz` keeps the full brake cycle). Next item is slack rope.

Update: slack rope and pick-up from the floor is built (blank `dh_floor_m` = rope always taut). Next item is heat over a duty cycle.
Update: heat over a duty cycle is built (Simulate > Drive run > Duty cycle). The hoist loop misbehaves at dt 10 ms (see PROGRESS). Next item is the slow-down before the final upper limit.
