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

## Next, in order
1. Agree with the panel thread how results become `Problem`s (new
   `ProblemKind`s, one `checks::calc_checks` call). Small PR first.
2. Adapter: read the model into calc inputs. Needs properties for
   installation method, ambient, grouping, supply voltage and source data
   (project properties), breaking capacity and trip curve on parts.
3. Motor starter adapter: find the starter chain (breaker, contactor,
   overload relay, motor) from the netlist.
4. PE continuity over the netlist (every PE terminal reaches the PE source).

## Open
- Table values are from memory of IEC 60364-5-52; verify against the
  edition in force before quoting numbers.
- XLPE method B2 column left out until verified.
