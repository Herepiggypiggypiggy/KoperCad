# T10 roadmap: simulation, drive, machine, controller flags

Source of rows: `handoff/problem-triangles/sites/sim.md` (89 rows on main 4ac0057).

| Step | State |
|---|---|
| Shared adapter and strip (`app/sim_flags.rs`), source `Sim`, `Life::Run(window)` | done (PR for lane T10) |
| Drive run and hoist run notes (left out, binding findings, refused start) | done |
| Winch scenarios (findings, failed run) | done |
| Motor start result (stalls, slow, cannot be modelled) | done |
| Controller card bindings: novo-sim `PinProblem` shown as a red triangle on the signal row | done |
| Census ceiling per file in `sim_flags_tests.rs` (98 -> 82) | done |
| Job flags (`Life::Until`) for save/read/export failures: faults.rs, controller_import.rs, controller_card.rs saves | next (T10b) |
| Faults study results (`never cleared` etc.), live.rs, live_anim.rs, motor_freq.rs | next (T10b) |
| machine.rs option refusals (transient flags on the tool) | next (T10b, with T7 refusal helper) |
| sim_chip.rs heat rows | T6 owns them |
| `ResistorFlag` and `check_drive_sizing` | no consumer anywhere; needs resistor power and duty cycle in the project (new spec keys, drive lane) |
