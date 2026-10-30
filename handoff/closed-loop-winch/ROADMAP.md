# Closed-loop and synchronous winches: roadmap

Design: `docs/design/closed-loop-winch.md`. Lane L2c of `docs/design/drive-crane-simulation.md`.

- Done: encoder, STO, closed-loop controller and hoist sequence, winch unit, design findings (`novo-drive::closed_loop`); two winches on a beam with rope stretch, tilt, position lock, torque follower, partner stop (`novo-machine::sync_winch`); six scenarios and the window Simulate > Winch scenarios.
- Done: schematic binding of one winch (property sheet section "Closed-loop winch", `cl_winch_spec`/`cl_winch_loop`, Drive run).
- Done: pair in the schematic (two motors, one beam, `cl_pair_loop`).
- Next, in order: (1) draw the ground and the slack rope in the 3D picture, and winches of a pair; (2) a rope that hangs at an angle from the start.
- Open: which drive family and encoder wiring the customer orders use (answer must come from the EPLAN automation project, not the repo); real brake coil timing from a data sheet; whether the position lock should use the drives' own sync function names.
