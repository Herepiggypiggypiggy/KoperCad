# Closed-loop and synchronous winches: handoff

Branch `claude/project-thread-a5ukwf` on kopercad. Resume: `cargo test -p novo-drive -p novo-machine`, then `cargo test -p novo-app winch`.

State: engines, scenarios and the window are built and tested (25 + 12 + 10 + 2 tests). Shared hoist model (`mechanics.rs`, `brake.rs`) belongs to the motor thread: this lane only added three getters to `SpringBrake` (`release_delay_s`, `torque_rise_s`, public `apply_delay_s`).

Where things are: `crates/novo-drive/src/closed_loop/` (controller.rs is the sequence), `crates/novo-machine/src/sync_winch.rs`, `winch_scenarios.rs`, window `crates/novo-app/src/app/winch_scenarios.rs`, command `sim.winches`.

Traps: the drive's torque proof is against the pre-torque asked for, not the clamped one (a load beyond the drive is never proven); guided beam statics make the healthy winch carry the whole weight when the other one is held; Shaft stiction means the held torque at stop sits inside [T_s eta, T_s / eta].
Decisions: scenarios use a generic 30 kW hoist; encoder plausibility uses true shaft speed above 10 % rated; no customer data in the repo.
