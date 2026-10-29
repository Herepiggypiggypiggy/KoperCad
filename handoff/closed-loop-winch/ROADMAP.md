# Closed-loop and synchronous winches: roadmap

Design: `docs/design/closed-loop-winch.md`. Lane L2c of `docs/design/drive-crane-simulation.md`.

- Done: encoder, STO, closed-loop controller and hoist sequence, winch unit, design findings (`novo-drive::closed_loop`); two winches on a beam with rope stretch, tilt, position lock, torque follower, partner stop (`novo-machine::sync_winch`); six scenarios and the window Simulate > Winch scenarios.
- Next, in order: (1) schematic binding: a motor property sheet section "Closed-loop hoist" and a `CircuitPort` reading the drive's run, STO and brake-relay devices so the live circuit drives `ClosedLoopWinch` like `hoist_run` does; (2) pair in the schematic (two motors, one beam); (3) load-cell and overload (SWL) input, slack-rope detection; (4) rope swing for non-vertical ropes; (5) 3D winch scene with the machine lane.
- Open: which drive family and encoder wiring the customer orders use (answer must come from the EPLAN automation project, not the repo); real brake coil timing from a data sheet; whether the position lock should use the drives' own sync function names.
