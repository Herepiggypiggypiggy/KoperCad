2026-10-29 12:00 UTC Started from the project owner's goal (niche and complex situations). Read plans and shared model.
2026-10-29 12:30 UTC Added `hoist_logic`, `drive_hoist_loop`, `drive_hoist_spec`, app section and Drive run integration; tests green.
2026-10-29 12:40 UTC Added fixture qa/fixtures/hoist-drive.novo (built by crates/novo-tests/examples/hoist_drive_fixture.rs) and QA scenarios 1709 and 1709b (both sizes) for Drive run with a drive hoist; Main B asked for UI evidence.

## Jump at reversal (roadmap item 1)
- `ReversalJump` in `hoist_logic` (events ReversalStarted/ReversalDone, ramp override), loop tracks the sag
  (`FindingKind::ReversalSag`), keys `dh_jump_hz` / `dh_jump_ramp_s`. 12 logic tests, 17 loop tests.
- Next: item 2 (slack rope and pick-up from the floor), from main in its own PR.
