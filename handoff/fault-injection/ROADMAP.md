# Fault injection and scenario simulation (lane F1) - roadmap

Scope: break the drawn live circuit on purpose and rehearse scenarios. Design `docs/design/fault-injection.md`.

## Done (PR 1)
- `novo_sim::inject`: defects (welded/stuck contacts, tripped, open/stuck coil, broken conductor/wire, lost supply, earth fault, short), `Faults`/`Defects` hooks in `settle_faulted` and `solve_with`.
- `novo_sim::walkthrough`: scripts, two-plant run, report (Markdown), recorder, JSON.
- App: Simulate > Defects and walk-throughs card, banner, record/run/delete, property `walkthroughs.project`.

## Next, in order
1. Machine-level defects through the same list: brake that does not hold (mechanics), lost encoder, drive overcurrent/STO, radio link loss; needs the drive-hoist, closed-loop and contactor-hoist threads' machine models and the controller thread (`CircuitPort` in `novo-machine`).
2. Scenario expectations on mechanics (hook height, speed, runaway) in the same Expect list.
3. Controller presses recorded and replayed (needs a controller action trace).
4. Starter library of generic walk-throughs (E-stop chain, welded reversing contactor, lost phase, brake open) shipped as lessons.
5. Defects in the Check menu as a "single fault" sweep: try every contact welded / coil open one at a time and list what breaks (FMEA-style table, with the honesty text).
6. PLC path: apply defects while a program runs.

## Open questions
- Which machine defects the hoist threads want first (answer from their EPLAN material).
