# Closed-loop and synchronous winches: roadmap

Design: `docs/design/closed-loop-winch.md`. Lane L2c of `docs/design/drive-crane-simulation.md`.

- Done: encoder, STO, closed-loop controller and hoist sequence, winch unit, design findings (`novo-drive::closed_loop`); two winches on a beam with rope stretch, tilt, position lock, torque follower, partner stop (`novo-machine::sync_winch`); six scenarios and the window Simulate > Winch scenarios.
- Done: schematic binding of one winch (property sheet section "Closed-loop winch", `cl_winch_spec`/`cl_winch_loop`, Drive run).
- Done: pair in the schematic (two motors, one beam, `cl_pair_loop`).
- Next, in order: (1) walk-through support for the winch and pair loops, a single lost encoder channel as a drawn device; (2) load-cell and overload (SWL) input, slack-rope detection; (3) rope swing for non-vertical ropes; (4) 3D winch scene with the machine lane.
- Open: which drive family and encoder wiring the customer orders use (answer must come from the EPLAN automation project, not the repo); real brake coil timing from a data sheet; whether the position lock should use the drives' own sync function names.
