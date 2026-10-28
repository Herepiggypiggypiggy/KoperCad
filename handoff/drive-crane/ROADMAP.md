# Drive and crane simulation: roadmap

Plan: `docs/design/drive-crane-simulation.md`. Crates: `novo-drive` (drive, motor at frequency, mechanics, brake) and `novo-machine` (crane kinematics, bindings, co-simulation).

- Done: L0 crate registration with API skeletons. L2a two-speed motor and hoist load (engine, tests; PR pending review).
- Next: L2a part 2 travel friction and inertia, L7 validation, C1 controller engine; then L5a loop, L4b Crane 3D, L3 drive data, L5b faults, L6 scope and checks. L1 travel drive engine follows. Lane order is in the plan.
- Open: the first-controller, receiver and closed-loop-hoist questions to Tadjiro are asked; the plan holds the defaults used until he answers.
