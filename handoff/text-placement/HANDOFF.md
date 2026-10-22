# Text placement handoff

Plan merged (#1351), stage 0 merged (#1354). Stage 0b PR on branch `claude/project-thread-img216`: `novo-canvas/src/text_audit.rs` (audit of a Scene),
scene dump key `text_audit`, `novo-tests/tests/text_baseline.rs` (ratchet 0/1/1).
Next: stage 1 engine `novo-canvas::textlayout` (pure; reuse `declutter::Boxes` grid, `note_place::{RINGS,SLIDES}`), stress generator for
the testers (terminals both sides, pitch 2.5 to 10 mm, rotations/mirror), QA scenario using scene dump `text_audit`.
The `text.conflict` check is deferred to stage 2 (checks are model based).
Traps: no stacked PRs; after READY do not push; cloud disk: CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0, test crates alone.

Stress kit (#1359, Text collision stress test thread) uses `text_audit::audit` for the shared measure: its `[tight]` is under 0.5 mm of air
(`text_audit::TIGHT`) and "outside the sheet" means the label box is not inside `Scene::sheet().rect`. After both land, re-check the 4,287 golden baseline once.

Stage 2a (#1367): neighbour causes rose on purpose (device tag / neighbour symbol line 1,715 -> 1,737, terminal name / neighbour symbol line 524 -> 642; mirrored terminals row page 100 -> 118) because tags now sit on the free side, facing the next symbol in tight packs. Stage 3 (terminal pitch and neighbours) must bring both below their pre-2a values (1,715 and 524). Net 4,287 -> 3,354.
