# Drive engine (lane L1): handoff

Branch `claude/project-thread-juy3ty`, one PR from main (70050541) touching only `crates/novo-drive` and this folder.
State: engine complete for its first slice, 24 integration tests plus unit tests pass, clippy and fmt clean.
Resume: `cargo test -p novo-drive` and `cargo clippy -p novo-drive --all-targets -- -D warnings`.

Files: `src/drive.rs` (re-exports, FIDELITY, LEAVES_OUT), `src/drive/{ramp,vf,bus,protection,sizing,terminals,engine}.rs`, `tests/drive_engine.rs`.

Coupling: the drive takes `motor_current_a` and `motor_power_w` from the motor model (lane L2) and gives frequency, voltage, torque limit back (`Drive::step(&DriveInputs, dt) -> DriveOutputs`). It never simulates the motor itself.

Decisions:
- Time 0 or a bad reference slope is an instant step; `dt <= 0` or NaN changes nothing; non-finite target holds.
- Energy balance of the bus is exact by construction (state is stored energy, every joule booked).
- V/f current limit holds the ramp; above 110 % of the limit it backs the frequency off at the decel slope.
- Vector mode is a steady-state torque limit (flux current from nameplate power factor), not current loops.

Hand cases un-ignored for L1 after PR 1555 merges: 2 (reversal, S-ramp), 3, 7, 8, 9 (drive part, ideal plant), 10. Case 9 still needs the motor torque-from-current relation from L2b.
Do not edit the expected values of PR 1555 without telling the L7 thread.
