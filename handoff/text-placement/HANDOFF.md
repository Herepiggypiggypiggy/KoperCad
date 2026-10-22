# Text placement handoff

Plan merged (#1351). Stage 0 PR on branch `claude/project-thread-img216`: `novo-canvas/src/text_audit.rs` (audit of a Scene),
scene dump key `text_audit`, `novo-tests/tests/text_baseline.rs` (ratchet 0/1/1).
Next: stage 1 engine `novo-canvas::textlayout` (pure; reuse `declutter::Boxes` grid, `note_place::{RINGS,SLIDES}`), stress generator for
the testers (terminals both sides, pitch 2.5 to 10 mm, rotations/mirror), QA scenario using scene dump `text_audit`.
The `text.conflict` check is deferred to stage 2 (checks are model based).
Traps: no stacked PRs; after READY do not push; cloud disk: CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0, test crates alone.
