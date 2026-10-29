# Open-loop drive hoist: roadmap

Scope (from the project owner, 2026-10-29): simulate niche and complex situations, not only common ones.
This lane: an open-loop frequency-drive hoist with brake logic, a brake contactor and rectifier in the
circuit, several speed steps and a load-dependent top step. Closed-loop hoists and contactor hoists are
other threads; the motor thread owns the shared model files.

Done
- `novo_drive::hoist_logic`: brake sequence, load sensing, faults (9 unit tests).
- `novo_machine::drive_hoist_loop`: loop, flux build-up, stiction with brake, findings (15 scenario tests).
- `novo_machine::drive_hoist_spec`: `dh_*` property keys (7 tests).
- App: property section "Drive hoist", Simulate > Drive run (readings, findings, badges), 2 app tests.
- Design section in `docs/design/drive-crane-simulation.md` ("Lane L2c-open as built").

Next, in order
1. (done) Jump at reversal: `dh_jump_hz`, `dh_jump_ramp_s`, finding ReversalSag.
2. Slack rope and hook-on-load detection (a load that is picked up from the floor).
3. (done) Heat over a duty cycle: `novo_drive::brake_heat`, `novo_machine::duty_cycle`, findings MotorTooHot, BrakeHot, BrakeWear, Simulate > Drive run > Duty cycle.
4. Ramp-to-limit slow-down switches for the upper limit (slow step before the final limit).
5. Closed-loop hoist (encoder, torque proving) in its own thread, sharing `HoistLogic` where it fits.

Open questions
- Real drive parameter sets cannot come from drawings: ask the owner for a typical set only if a run
  must match a specific machine (kept out of the public repo).
