# Drive and crane simulation: roadmap

Plan: `docs/design/drive-crane-simulation.md`. Crates: `novo-drive` (drive, motor at frequency, mechanics, brake) and `novo-machine` (crane kinematics, bindings, co-simulation).

- Done: L0 crate registration with API skeletons. L1 travel drive engine (#1557). L2a two-speed motor and hoist load (engine, tests; PR pending merge).
- Next: L2a part 2 travel friction and inertia, L7 validation, C1 controller engine; then L5a loop, L4b Crane 3D, L3 drive data, L5b faults, L6 scope and checks. Lane order is in the plan.
- Open: whether any hoist is a closed-loop drive hoist (lane L2c) and which controllers ship first are still unanswered; until then the engine assumes two-speed contactor hoists.
- L7: hand-calculation suites for cases 1-21 are in; they are un-ignored as lanes land (doc `docs/design/drive-crane-validation.md`). The fixture crane project is shared with controller lane C7.
