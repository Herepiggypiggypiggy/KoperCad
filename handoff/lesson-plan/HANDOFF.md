# Lesson plan and learning menus: handoff

State (2026-10-19): PRs 1 to 3 of the plan are merged (#583, #674, #958); PR 4 (paths) is in review on branch `claude/learning-paths`; see PROGRESS.md.
Pure logic lives in `novo-learn` (`Catalog::next_lesson`, `unmet_needs`, `track_summary`, `path_lessons`, `path_summary`, `path_next`; paths come from `assets/lessons/paths.toml`); the app only
draws it (`learn_ui.rs`, `menu_bar.rs` `paths_menu` and `command_label`).

Resume: branch from main, `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p novo-app -p novo-learn -p novo-workflow -p novo-tests`. Use `CARGO_INCREMENTAL=0
CARGO_PROFILE_DEV_DEBUG=0` and `cargo clean` when the disk fills.

People: reviewer R4 (live view, Learn, commissioning) first for lesson work; merge lead A merges.
Learning runner thread owns the replay gate and lint. Roster: `coordination/threads.md`.

Traps and decisions:
- My development branch name is shared with a closed PR; open new PRs from a new branch name and never
  force-push a branch whose PR is READY or held.
- Once READY, do not push; follow-ups go in a new PR (merging main when a lead asks is the exception).
- A new command needs: `commands.rs`, `language.rs` (NL), `menus.rs` placement, `APP_COMMANDS` in
  `workbench.rs`, and a handler in `run_command`.
- Lessons that name UI not on main stay `draft` or `early`; the runner lints free-text button names.
