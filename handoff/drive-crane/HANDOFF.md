# Drive and crane: handoff

Branch `claude/project-thread-inah5y`, PR "Register novo-drive and novo-machine crates".
State: skeletons only. `novo-drive` has no dependencies yet (lanes add `novo-core`/`novo-sim` in their own Cargo.toml; avoid a cycle with L5 hooks in `novo-sim`). `novo-machine` depends on `novo-drive`.
Resume: `cargo test -p novo-drive -p novo-machine`.
Owner for new lanes: see the plan lane table.

L7 validation: `docs/design/drive-crane-validation.md` lists every case, what runs now and which lane un-ignores which test. A lane that lands engine code replaces the `todo!` in its tests and keeps the hand value; disagreements go to the doc, not to a changed hand value.
