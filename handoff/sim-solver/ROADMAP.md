# Real simulation: analog solver (brief 8a, docs/AMBITION-REVIEW.md)

Scope: MNA analog solver (DC, AC phasors, transient), transformer, induction motor start model, the
drawing -> circuit extractor and the app entry. Lanes 8b (PLC program run) and 8c (fault scenarios, trip
curves, waveform view) are separate threads (8c: session cse_01NQC5mMueppdFcRDgtrhhwd) that consume this API.

## Done
- `novo_sim::analog`: Circuit/Element/Wave, `solve_dc`, `solve_ac`, `transient`, dense complex LU with row equilibration. 18 textbook tests.
- `novo_sim::analog::motor`: equivalent circuit, nameplate fit, direct / star-delta / auto-transformer / soft start, load models. 12 tests.
- `novo_sim::network::analyse`: sources from potentials (L1..L3 phase, `short_circuit_ka` source impedance), closed poles, motors and resistive loads with notes for assumed/missing data. 5 tests.
- App: live view (Shift+L) shows volts per wire in the hover tip, amps and kW per load in the card, "Start -M1..." button; Ctrl+K "Simulate motor start…" window with speed and current plot (`app/simulate.rs`).

## Next
1. Cable resistance per wire once wire lengths feed the model (wire list cut lengths), then true voltage drop.
2. Latching fixpoint check in `settle` (seal-in circuits); transformer and PSU symbols as sources in `analyse`.
3. Non-linear parts (diodes, saturation); motor in the transient solver (dq model) for inrush peaks.
4. Textbook validation of the motor against a published example (Chapman) once the book values are at hand.

## Open
- Non-linear parts are not modelled; conductors are ideal.
