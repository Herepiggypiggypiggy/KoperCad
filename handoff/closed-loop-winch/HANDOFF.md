# Closed-loop and synchronous winches: handoff

Branch `claude/project-thread-a5ukwf` on kopercad. Resume: `cargo test -p novo-drive -p novo-machine`, then `cargo test -p novo-app winch`.

State: engines, scenarios and the window are built and tested (25 + 12 + 10 + 2 tests). Shared hoist model (`mechanics.rs`, `brake.rs`) belongs to the motor thread: this lane only added three getters to `SpringBrake` (`release_delay_s`, `torque_rise_s`, public `apply_delay_s`).

Where things are: `crates/novo-drive/src/closed_loop/` (controller.rs is the sequence), `crates/novo-machine/src/sync_winch.rs`, `winch_scenarios.rs`, window `crates/novo-app/src/app/winch_scenarios.rs`, command `sim.winches`.

Traps: the drive's torque proof is against the pre-torque asked for, not the clamped one (a load beyond the drive is never proven); guided beam statics make the healthy winch carry the whole weight when the other one is held; Shaft stiction means the held torque at stop sits inside [T_s eta, T_s / eta].
Decisions: scenarios use a generic 30 kW hoist; encoder plausibility uses true shaft speed above 10 % rated; no customer data in the repo.

Circuit binding (second PR): `crates/novo-machine/src/cl_winch_spec.rs` (keys `cw_*`), `cl_winch_loop.rs`, `crates/novo-app/src/app/drive_run.rs` (rows, findings, badges), inspector section in `inspector.rs`, fixture `crates/novo-tests/src/closed_winch.rs` and `qa/fixtures/closed-winch.novo` (regenerate: `cargo run -p novo-tests --example closed_winch_fixture -- qa/fixtures/closed-winch.novo`). Resume: `cargo test -p novo-machine --test cl_winch_loop` and `cargo test -p novo-app --lib drive_run`.
Traps: the loop runs at the app's 5 ms step and the drive is stable there; STO and the encoder are coils in the drawing (energised = ok); an unbound STO channel or encoder device counts as always present and check_bindings says so; cargo `build/*/out` must not be deleted to free disk (breaks the shared target).

