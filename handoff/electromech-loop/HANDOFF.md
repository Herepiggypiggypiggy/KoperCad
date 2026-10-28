# L5a handoff (2026-10-28)
Branch claude/project-thread-agxv7b.
PR 1 (merged, #1561): `novo_machine::port` (CircuitPort, FakeCircuit, MotorFeed), `novo_machine::hoist_loop` (HoistLoop, HoistModule, HoistBinding, LimitSwitch), `TwoSpeedHoistUnit::step_energised`.
PR 2 (Hoist run in the live view): `novo_machine::spec` (mech_* properties), step config validation, `HoistLoop::check_bindings`, `novo-app` `app/hoist_run.rs` (LiveCircuit adapter, Hoist run window, badges), property sheet "Mechanical load", command `sim.hoist` (Simulate > Hoist run), fixture `qa/fixtures/hoist-two-speed.novo` (built by `crates/novo-tests/examples/hoist_fixture.rs`), QA scenarios `600-hoist-run` and `600-hoist-run-1920`.
Known model limits: the generic Kloss fit gives the fine winding about 5 N m start torque, so the fixture hook is 100 kg; a real high-torque hoist motor lifts more. Limit tags in properties are the full device tag string.
Next (PR 3): drive module in the loop (Drive::step with motor current feedback, "Drive" section, Simulate > Drive run) after L2b. Then bindings stored in the project model (ask WS-02), controller panel signals (C2), a checks entry for binding problems.
Customer facts (plans/crane-setup-facts.md) stay out of public files.
