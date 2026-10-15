# Onboarding: handoff
Thread: "Sample project and first-run walk-through" (session_019sSSXcKhY8k5uYwY3cgSyf). Branches claude/project-thread-e4qih3 (docs #373), ...-example (#394), ...-tour (walk-through PR, stacked on the example).
State: docs #373 and example #394 merged; the walk-through PR is the open one (branch claude/project-thread-e4qih3-tour). Reviewers: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W, design reviewer cse_0114emQ5fSFrzcALbR4S18TU. Integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S merges. #392 is superseded.
Code: crates/novo-app/src/app/example.rs, onboarding.rs, tests example_tests.rs, onboarding_tests.rs. Settings: `Settings.onboarding` (ticked, dismissed, started). Actions: OpenExample, TryFastKeys, ShowWalkthrough; commands file.example, file.practice, help.walkthrough in novo-workflow.
How ticking works: tool state each frame (Place/Wire/Cable via `tour_watch`), Tab/Enter with proposals, `run_command` for duplicate, palette, `toggle_live` for live view, `open_wheel`/Q key for place. Editor keys (W, Tab) do not pass run_command, which is why state is watched.
Traps: the example depends on novo-tests (`motor_starter_in`); keep it a thin wrapper. Disk: target/ grows to 26 GB; use CARGO_INCREMENTAL=0 and delete target/debug/deps if "no space".
Resume: git fetch origin; git checkout claude/project-thread-e4qih3-tour; cargo test -p novo-app onboarding.
