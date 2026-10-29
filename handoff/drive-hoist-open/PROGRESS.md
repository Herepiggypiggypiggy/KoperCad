2026-10-29 12:00 UTC Started from the project owner's goal (niche and complex situations). Read plans and shared model.
2026-10-29 12:30 UTC Added `hoist_logic`, `drive_hoist_loop`, `drive_hoist_spec`, app section and Drive run integration; tests green.
2026-10-29 12:40 UTC Added fixture qa/fixtures/hoist-drive.novo (built by crates/novo-tests/examples/hoist_drive_fixture.rs) and QA scenarios 1709 and 1709b (both sizes) for Drive run with a drive hoist; Main B asked for UI evidence.

## Jump at reversal (roadmap item 1)
- `ReversalJump` in `hoist_logic` (events ReversalStarted/ReversalDone, ramp override), loop tracks the sag
  (`FindingKind::ReversalSag`), keys `dh_jump_hz` / `dh_jump_ramp_s`. 12 logic tests, 17 loop tests.
- Next: item 2 (slack rope and pick-up from the floor), from main in its own PR.

## Slow-down before the final limit (roadmap item 4)
- Limit tracks in the loop (4 findings), `dh_slow_*` and `dh_clearance_*` keys (1 spec test), 3 loop tests, 1 app test with the drawn fixture `qa/fixtures/hoist-drive-limits.novo` (`drive_hoist_with_limits`), QA 1760 / 1760b.
- Roadmap items 1 to 4 are built. Open: item 5 (closed-loop hoist) is a separate thread.
## Heat over a duty cycle (roadmap item 3)
- `novo_drive::brake_heat` (4 tests), heat in the loop (`with_heat`, 3 loop tests), `dh_heat_*` and `dh_brake_*` keys (1 test), `novo_machine::duty_cycle` (5 tests), Drive run window section with a background job (1 app test), QA 1750 / 1750b.
- Found on the way: the hoist loop at a 10 ms step cannot start a lowering from rest after a lift (current stuck at the locked-rotor value); the duty run refuses steps over 5 ms. Not investigated.
- Next: roadmap items 1 to 4 are done; open: the hoist loop at dt 10 ms.
