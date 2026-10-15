# Electrical calculations: handoff

Updated 2026-10-15 by thread "Design checks: electrical calculations"
(session cse_011d2XMxLEdY3akxWeLHGAGn).

## State
- #412 (pure library `novo_checks::calc`) is merged to main.
- #454 (adapter) was merged into the stale #412 branch, not main. Re-landed
  as #514 (branch `claude/project-thread-bib2al`, from main): adapter,
  six ProblemKinds, `Source::from_transformer_min`, shared drop limit.
  Needs reviewer READY on its current head, then merge.
- Panel thread's #413 carries VoltageDrop/ShortCircuit as label-only kinds:
  on conflict keep one copy of each variant and merge the CALC_KINDS skip
  list in tests/motor_starter.rs with theirs.
- UI integration thread builds "Project settings > Electrical" (supply.*,
  check.voltage_drop_pct) after #514.
- Not started: cable rating with installation method (replace the
  single-column ampacity in checks::undersized_wires), short-circuit
  current along cables, PE continuity over the netlist, "planning value,
  verify" wording on table-based results, verifying the IEC tables.

## Resume
```
git fetch origin main
git checkout -B claude/project-thread-bib2al origin/main
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
