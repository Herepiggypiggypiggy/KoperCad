# Crane project template: handoff

Branch `claude/project-thread-niw9my` in kopercad. Files: `crates/novo-reports/src/front.rs`, `crates/novo-app/src/app/template_crane.rs` (+ `_tests.rs`), `project_settings.rs` (Project information section), `example.rs` (`new_crane_project`, `open_built`), `actions.rs`/`menus.rs`/`commands.rs` (`file.new_crane`), core `FRONT_GENERATOR` in `project.rs`.

How to resume: `cargo test -p novo-reports -p novo-app template_crane`. Use `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` on a small disk.

Decisions
- Own tags (`=CRN+GIR+MCC`, `+HU1+CTL`, `+FLD+LSB`...); no customer ids or EPLAN content. Generic crane facts only.
- Cabinet codes sort in drawing order (DST before MCC), because pages sort by structure first.
- Front matter is one report with five sections so the standard report list grows by one, not five.
- A template is a new unsaved project (not marked saved), unlike examples.
