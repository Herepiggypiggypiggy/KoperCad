# Handoff: Simulation: faults, trip curves, waveforms

Branch `claude/project-thread-an4tyc` (kopercad). PR: see PROGRESS.md for number and head SHA.

## State
Engine and window are in one PR. Everything is under `crates/novo-sim` (modules `trace`, `fault`, `protection`, `scenario`, `network`, `timing`, `study`) plus `crates/novo-app/src/app/faults.rs`. The command is `sim.faults` (palette: "Fault scenarios"; Dutch words in language.rs).

## How to resume
```
cargo test -p novo-sim            # tests/faults.rs and tests/logic_timing.rs
cargo run -p novo-app             # palette (Ctrl+K) "fault scenarios"
```
The window reads `<project>.faults.json` next to the saved project, or shows `Study::example()`. Save study / exports need a saved project.

## Interfaces agreed
- Curves: consume `novo_checks::calc::curves` (Protection and cable engineering thread, cse_01CQXymGH5kY1GfABR1JHePK) via `protection::TripModel` (`trip_time_range`, `clears`, `let_through_i2t`). No second curve model; `TableCurve` is only for printed manufacturer curves.
- Solver: lane lead cse_015JRbxiYcLVW7Zr22rGssbA owns `novo_sim::analog`. Plan: implement `scenario::CircuitModel` on top of it. `trace::Trace` is offered as the shared time-series type.
- Selectivity result offered to Protection coordination and selectivity (cse_01AVPuavJqU7fun5pSgYa9vV).

## Traps and decisions
- A tripped device conducts until the first natural current zero after `opening_time`; the waveform and let-through use that, not the trip decision time.
- Bands: `Fastest` trips at t_min, `Slowest` at t_max; in a curve's "may or may not trip" gap (t_max infinite) only Fastest trips.
- Thermal memory is cumulative damage (dt / t_trip) with exponential cooling when below pickup; honest approximation, say so in the UI.
- Radial model leaves out motor contribution and AC decay; the solver lane adds them.
- Test curves in tests and the example study are shaped like IEC 60898/60269/60947-4-1 but are not manufacturer data.
