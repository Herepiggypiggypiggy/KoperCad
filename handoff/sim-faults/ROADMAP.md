# Simulation: faults, trip curves, waveforms (lane c of Real simulation)

Scope: fault scenarios, protection operating in time, timing diagram and waveform view. Lane lead: "Real simulation: solver and faults" (analog solver API). Curve model: Protection and cable engineering thread (`novo_checks::calc::curves`). Scenario files, the runner and the views are this lane's.

## Done (PR from branch claude/project-thread-an4tyc)
- `novo_sim::trace`: Trace/TraceSet, rms/peak/i2t/crossings, min/max decimation, CSV.
- `novo_sim::fault`: IEC 60909 kappa, asymmetrical current i(t), interruption at the next current zero, let-through.
- `novo_sim::protection`: `TripModel` trait (curve source), `Band` (fastest/typical/slowest), `TableCurve`, cumulative-heat thermal memory.
- `novo_sim::scenario`: JSON scenario file, event-driven runner, selectivity result, `CircuitModel` trait.
- `novo_sim::feeder`: radial feeder model (Ik3, Ik2, Ik1 phase-neutral/earth, open phase = x sqrt 3).
- `novo_sim::timing`: digital channels + analog rows, SVG and VCD export, `record_logic` for the drawing's own control logic.
- `novo_sim::study`: `<project>.faults.json` study (network, devices, scenarios) with an example.
- App: `sim.faults` "Fault scenarios" window.

## Next (in order)
1. `impl TripModel` for the novo-checks curve device once that API lands; `CurveSpec::Generic` resolved through it.
2. `impl CircuitModel` on the analog solver (meshed networks, motor contribution, decaying AC component); take FaultCurrent from the solver.
3. Build the study from the drawing (cables with length and part, protective devices with parts) instead of the file.
4. Instantaneous waveform of the full transient from `transient()` traces in the diagram; cursors with readout.
5. Scenario editor in the window (add fault, motor start) instead of editing the JSON.
6. Backup protection time (breaker failure), current-limiting let-through from the curve model's i2t.
7. Scenario runs in `kopercad-cli` and as a Python call (WS-28).

## Open questions
- Does the protection thread want `ScenarioResult` selectivity as its time-domain check? (Offered; no answer yet.)
