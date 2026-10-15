# Electrical calculations (novo-checks::calc): roadmap

## Scope
Engine side of the design checks: electrical calculations as a pure library
in `crates/novo-checks/src/calc/` (numbers in, `Finding`s out, no Project).
The panel thread ("Design checks panel") owns the panel UI, structural
checks, waivers and the report. Draft PRs, no build.

## Done
- PR 1: the pure library. cable (ampacity tables B1/B2/C, PVC and XLPE
  (no XLPE B2), ambient and grouping factors, `Ib <= In <= Iz`), drop
  (voltage drop, longest length), short_circuit (source from Ik or
  transformer, ik3 max, ik1 min, breaking capacity, disconnection by curve,
  thermal withstand), motor (full-load current, starter checks), pe
  (section rule, adiabatic, loop impedance, path resistance).

- PR 2 (stacked on #412): six ProblemKinds (VoltageDrop, ShortCircuit,
  DisconnectionTime, ThermalWithstand, MotorStarter, ProtectiveConductor)
  and `calc_checks.rs`, the adapter reading properties (table in its module
  doc). Motor starter chain found by walking even-numbered pins upstream.

## Next, in order
1. Cable rating with installation method, ambient and grouping
   (`calc::cable::Circuit`) replacing the single-column ampacity in
   `checks::undersized_wires`; map Rule::CableRating to UndersizedWire.
2. Short-circuit current reduced along cables (needs the feeder chain).
3. PE continuity over the netlist (every PE terminal reaches the PE source).
4. "Planning value, verify" wording on table-based results once the cable
   rating check uses the calc tables.

## Open
- Table values are from memory of IEC 60364-5-52; verify against the
  edition in force before quoting numbers.
- XLPE method B2 column left out until verified.
