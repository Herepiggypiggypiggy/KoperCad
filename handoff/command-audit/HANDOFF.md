# Command and Learn consistency audit: handoff

Branch `claude/project-thread-unuj27` (PR from main). Tests live in `crates/novo-app/src/app/tips_tests.rs` (handler list drift) and `learn_replay.rs` (clicks and pointer rings). The audit text is `audit/commands-and-lessons.md` on the project mount.

Resume: `cargo test -p novo-app --lib -- the_handler_match every_shipped_lesson_replays`.

Traps: the replay cannot always deliver a press in the headless window (a stale window can swallow it), so a click that does not tick is raised as an event and counted as synthetic; later pointer checks in that lesson are then skipped. `KNOWN_BROKEN` entries must be removed by whoever fixes the step.

Talk to: door/hinge thread cse_01EyyUrjvpHBkCtJRxaRXQYc, lessons threads cse_015yG1UiqCcrM7L2vKms4dXz and cse_01GtrgYmDXm75UfvZzUV67rz, reviewer R4 cse_01R45nLDrXZPS8rKtX6RjHJR.
