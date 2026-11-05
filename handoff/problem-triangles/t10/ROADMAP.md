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
| Job flags for save/read/export failures: faults.rs (read, save, export, store), controller_import.rs (profile save, table read) | done (T10c, third PR). controller_card saves have no failure toast on main |
| Controller card (damaged, notes, checks, shared targets, card and popped panels), Live card (no supply, short circuit), motor at frequency | done (T10b, second PR; census 82 -> 70) |
| Faults study results (`never cleared` etc.), live_anim.rs canvas colours | remaining |
| machine.rs option refusals (transient flags on the tool, `refuse_tool`) | done (T10c) |
| sim_chip.rs heat rows | T6 owns them |
| `ResistorFlag` and `check_drive_sizing` | no consumer anywhere; needs resistor power and duty cycle in the project (new spec keys, drive lane) |
