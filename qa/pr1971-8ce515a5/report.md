# PR 1971 QA, source at git 8ce515a5056fddba2edf7ff765be29a1663cf18c

Scenarios 9609 (1366x768) and 9610 (1920x1080): open the Travel drive project, open Hoist in 3D, take the generic crane, answer the file dialog with the synthetic crane `qa/fixtures/crane-model/test-crane-parts.stp` (six named boxes, made by `make_fixture.py`), press **Use my crane model...**, expect "5 of 6 parts used", take the picture with the model, press Hook, then **Remove model** and take the generic crane again. `check_layout` passes at both sizes. QA gate: PASS (2 runs, 0 failed, 0 findings).

Pictures: `/mnt/project-files/qa/pr1971-8ce515a5/shots/` (generic-crane, own-model-in-the-picture, own-model-hook-framed, generic-again, both sizes). The status line "5 of 6 parts used" fits on one line at 1366.

Tests: cargo test -p novo-interop --lib crane_model (4), cargo test -p novo-view3d --lib crane (30), clippy -D warnings clean on novo-view3d, novo-interop, novo-app, novo-cli. The tip commit adds only evidence and handoff notes.

Known: the pieces are boxes here because the fixture is synthetic; the drum and sheaves do not turn yet (part 3).
