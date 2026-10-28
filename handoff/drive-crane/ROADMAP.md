# Drive and crane simulation: roadmap

Plan: `docs/design/drive-crane-simulation.md`. Crates: `novo-drive` (drive, motor at frequency, mechanics, brake) and `novo-machine` (crane kinematics, bindings, co-simulation).

- Done: L0 crate registration with API skeletons and one test per crate.
- Next: L1 drive engine, L2 motor and mechanics, L7 validation in parallel; then L3, L5; then L4, L6.
- Open: questions to Tadjiro listed at the end of the plan.
