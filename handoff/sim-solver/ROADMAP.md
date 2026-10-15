# Real simulation: analog solver (brief 8a, docs/AMBITION-REVIEW.md)

Scope: MNA analog solver (DC, AC phasors, transient), transformer, induction motor start model, then the
Project -> Circuit extractor and the Simulate workspace entry. Lanes 8b (PLC program run) and 8c (fault
scenarios, trip curves, waveform view) are separate threads that consume this API.

## Done
- `novo_sim::analog`: Circuit/Element/Wave, `solve_dc`, `solve_ac`, `transient`, dense complex LU. 17 textbook tests.
- `novo_sim::analog::motor`: equivalent-circuit motor, nameplate fit, direct / star-delta / auto-transformer / soft start, load models. 12 tests.

## Next
1. Project extractor: potentials -> sources (voltage, frequency, phase), closed poles -> switches, wires -> resistance from cross-section and length, loads from device properties.
2. Simulate workspace: voltage/current annotations on live view (Shift+L), motor start plot.
3. Latching fixpoint check in `settle` (seal-in circuits).

## Open
- Non-linear parts (diodes, saturation) are not modelled.
