# Electrical calculations: handoff

Updated 2026-10-15 by thread "Design checks: electrical calculations"
(session cse_011d2XMxLEdY3akxWeLHGAGn).

## State
- Branch `claude/project-thread-bib2al`, draft PR "Checks: electrical
  calculation library" (see PROGRESS.md for the number and head).
- Only new files under `crates/novo-checks/src/calc/` plus one line
  (`pub mod calc;`) in `novo-checks/src/lib.rs`. No model reading yet, no
  ProblemKind changes.
- `cargo test -p novo-checks`, clippy -D warnings and fmt pass.

## Resume
```
git fetch origin claude/project-thread-bib2al main
git checkout claude/project-thread-bib2al
cargo test -p novo-checks calc
```

## Adapter PR
Branch `claude/project-thread-bib2al-adapter` (stacked on #412): kinds in
lib.rs, `calc_checks.rs`, tests/calc_checks.rs. The Gate 1 test in
tests/motor_starter.rs skips the six calc kinds (they need numbers).
Panel PR #413 also adds VoltageDrop/ShortCircuit: expect a small enum merge.

## How to reach it (user-visible entry)
Results appear as problems in the Issues list (design checks panel, #413):
kinds VoltageDrop, ShortCircuit, DisconnectionTime, ThermalWithstand,
MotorStarter, ProtectiveConductor; waivable like any problem. Inputs are
properties: project `supply.short_circuit_ka`, `supply.voltage_v`,
`check.voltage_drop_pct`; device/part `rated_current_a`,
`breaking_capacity_ka`, `trip_curve`, `let_through_i2t`, relay
`setting_min_a`/`setting_max_a`, contactor `ac3_current_a`, `max_scpd_a`;
wire/cable `length_m`, `load_current_a`, `insulation`. Settings section
("Project settings > Electrical") proposed to the UI integration thread
(session_01TaTTPuTqhk8bHd277oJRFR) on 2026-10-15.

## Decisions
- Pure functions on numbers so the same code serves checks, property sheet
  and Python. `Finding { rule, severity, message }`; `Rule` is the stable
  key for waivers.
- Findings only when something is wrong; a missing input says nothing.
- Tables outside copper 1.5 to 120 mm² return `None`.

## Traps
- Table numbers are not verified against the standard text (offline).
- The panel thread may reshape `Problem`; do not add ProblemKind variants
  before agreeing with it.

## Who to talk to
Panel thread cse_0191A95sdns9gs3sLWgmXiis; integration lead
cse_01FiLvRBKEcNNHbQEGhYyL1S (roster: coordination/threads.md).
