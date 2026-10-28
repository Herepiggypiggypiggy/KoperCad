# Drive and crane: handoff

Crates `novo-drive` and `novo-machine` are on main (L0). L2a adds the two-speed hoist engine in `novo-drive` (`motor`, `sequencer`, `brake`, `mechanics`, `hoist_unit`, `error`).
Entry point for the loop lane (L5a): `hoist_unit::TwoSpeedHoistUnit::step(HoistInput, dt)` returns a `HoistSnapshot`; the schematic supplies `Command` (direction, step 0/1/2), `mains_on` and, with `BrakeFeed::Separate`, the brake coil.
Brake torque of a tick is the value at its start (one tick of latency, deterministic).
Conventions: positive shaft speed raises the hook; see `mechanics.rs` header for the efficiency convention.
Resume: `cargo test -p novo-drive -p novo-machine`.
No UI entry yet: the "Mechanical load" section of the motor property sheet (L3/L6) and the loop (L5a) consume it.
