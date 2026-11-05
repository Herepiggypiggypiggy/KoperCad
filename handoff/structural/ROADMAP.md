# Structural estimates (novo-checks::calc::mechanical): roadmap

## Scope
Closed-form mechanical estimates, own formulas, no CalculiX code (GPL).
Source: research/calculix/SUMMARY.md, adopt row 2.

## Done
- Engine `crates/novo-checks/src/calc/mechanical.rs`: peak factor, force
  between parallel conductors (two conductors, three-phase middle bar),
  bending stress and longest span for three support layouts, support
  force, natural frequency and resonance flag; beam deflection (simple,
  clamped, cantilever; spread and point loads; top-hat rail section
  computed from its rectangles); simply supported plate by Navier series.
- UI: Check > Calculations > Mechanical tab (`calculations_mechanical.rs`),
  copy as text, "model, closed form" label (Help > About the numbers).

## Next
- Take busbar data from the model once busbars are drawn (rated current,
  bar size, support part ratings) instead of typed inputs.
- Device weights in the part catalog, so rail and plate loads fill in
  from the placed devices.
- A ProblemKind in the Issues list for a failing busbar (needs the model data above).
- Dynamic factor of IEC 60865-1 (resonance curve), sub-conductors, Dwight shape factor.
- Thermal short-circuit withstand of the bar (adiabatic, I^2 t).

## Open
- Clause numbers of IEC 60865-1 unverified; material yield values are typical, not certificates.
