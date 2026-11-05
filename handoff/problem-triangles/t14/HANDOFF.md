# T14 Shell flags: handoff

Branch `claude/project-thread-m14aux`. Scenarios 9540 (1366x768), 9541 (1920x1080).

## What exists
- `crates/novo-app/src/app/flags_shell.rs`: the shared shell surface.
  - `job_flag(rule, red, title, detail, fix)` / `job_ok(rule)`: a failed job is a flag (`Source::Shell`, `Anchor::Job`, `Life::Until`). Replaces a red/yellow toast. A status bar chip (triangle, count) opens the popover; popover has the fix command, Copy, Dismiss.
  - `control_flag(key, rule, ...)`, `control_flags(key)`, `flag_row(ui, &flags)`, `control_clear_all(key)`: flags on a window (`Anchor::Control`), drawn at the top of that window; cleared when the window closes.
  - `inline_flag(ui, red, dashed, title, detail)`: a recomputed state (add-on stopped, language gaps, step text broken) as triangle + short title, detail in a popup. No store.
  - `COMMANDS`: command ids a shell flag may offer as Fix/Open (must be in `workbench::APP_COMMANDS`).
- Migrated: diagnostics, userkeymap, describe_recipes, languages, learn, onboarding, preferences, centre, drawers, project_rules, project_settings, add-ons (9 banners + stop toast), datasheet review.
- Census test `flags_shell_tests::shell_census_ceiling` (ceiling 2): left on purpose are an informational add-on banner and the datasheet "sends the whole file" consent text (class T).

## Left alone on purpose
- `main.rs`, `dialogs.rs`, `startup.rs`: startup failures stay plain `rfd` dialogs (the flag UI does not exist yet there).
- `novo-learn` lint stays CI text; `learn_suite.rs` (lesson suite problems, test-only list).
- `status_bar.rs`, `single_line_view.rs`, `autopilot.rs`, `novo-ai`: listed in `sites/shell.md` but owned by other lanes/files.
- `update_check.rs`: no problem sentences of its own.

## Traps
- Another lane that adds a job chip to the status bar should reuse `shell_chip()`; T12 may want `Source::Job` flags in the same chip.
- The scenarios set `PATH=/nonexistent` so `xdg-open` cannot start; that is the failing job.
