# Drive and crane: handoff

Branch `claude/project-thread-inah5y`, PR "Register novo-drive and novo-machine crates".
State: skeletons only. `novo-drive` has no dependencies yet (lanes add `novo-core`/`novo-sim` in their own Cargo.toml; avoid a cycle with L5 hooks in `novo-sim`). `novo-machine` depends on `novo-drive`.
Resume: `cargo test -p novo-drive -p novo-machine`.
Owner for new lanes: see the plan lane table.
