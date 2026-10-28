# Drive and crane simulation: roadmap

Plan: `docs/design/drive-crane-simulation.md`. Crates: `novo-drive` (drive, motor at frequency, mechanics, brake) and `novo-machine` (crane kinematics, bindings, co-simulation).

- Done: L0 crate registration with API skeletons and one test per crate.
- Next: L1 drive engine, L2 motor and mechanics, L7 validation in parallel; then L3, L5; then L4, L6.
- Open: see Open questions in the plan.
- L7 done for cases 1-21 as oracles; later: un-ignore as lanes land, fixture crane project shared with controller lane C7.
