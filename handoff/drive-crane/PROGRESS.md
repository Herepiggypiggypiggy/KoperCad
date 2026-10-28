2026-10-28 08:40 UTC L0: registered novo-drive and novo-machine, skeletons and tests, CLAUDE.md table, plan copied to docs/design.

2026-10-28 09:41 UTC L7: hand-calculation suites added (novo-drive and novo-machine tests/hand_validation.rs, docs/design/drive-crane-validation.md). 27 tests run, 22 wait for lanes and are ignored with the lane named. Findings F-1 (Ramp reversal through zero), F-2, F-3 in the doc.
2026-10-28 L2a: two-speed pole-change motor (Kloss fit), contactor sequencer, rectifier-fed spring brake, hoist gravity with direction-dependent losses, stiction shaft, `TwoSpeedHoistUnit`, `InputError` validation (poles 0, ratio or efficiency 0, negative length). 23 hand-calculated tests in `crates/novo-drive/tests/motor_and_load.rs`. Not in this PR: travel friction and inertia (L2a part 2), current and thermal (L2b), UI.
2026-10-28 08:24 UTC L0: registered novo-drive and novo-machine, skeletons and tests, CLAUDE.md table, plan copied to docs/design.

2026-10-28 L7: merged main with L1 (#1557); un-ignored the L1 cases 2 (reversal, S-ramp), 3, 7, 8 and 10, written from my own hand values; all pass. F-1 and F-3 are fixed in L1. Case 9 waits for L2b. #1556 (L2a) will need one more adaptation.

2026-10-28 L7: merged main with L2a (#1556); adapted the tests to the Result API and hoist_raising_torque_nm; no expected value changed. L2a cases 5, 6, 13, 14 are still ignored: a follow-up PR un-ignores them after #1555 merges.
