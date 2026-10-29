# Crane project template: handoff

Branch `claude/project-thread-niw9my` in kopercad. Files: `crates/novo-reports/src/front.rs`, `crates/novo-app/src/app/template_crane.rs` (+ `_tests.rs`), `project_settings.rs` (Project information section), `example.rs` (`new_crane_project`, `open_built`), `actions.rs`/`menus.rs`/`commands.rs` (`file.new_crane`), core `FRONT_GENERATOR` in `project.rs`.

How to resume: `cargo test -p novo-reports -p novo-app template_crane`. Use `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` on a small disk.

Decisions
- Own tags (`=CRN+GIR+MCC`, `+HU1+CTL`, `+FLD+LSB`...); no customer ids or EPLAN content. Generic crane facts only.
- Cabinet codes sort in drawing order (DST before MCC), because pages sort by structure first.
- Front matter is one report with five sections so the standard report list grows by one, not five.
- A template is a new unsaved project (not marked saved), unlike examples.

Follow-up (after #1719 merged)
- `file.new_crane` now opens the New crane project window (`template_crane_window.rs`, `wb.crane_new`); Create calls `new_crane_project(options)`.
- `template_crane.rs`: `CraneOptions` (hoist_units 1..=4, hoist_drive, travel_drive); pages place circuits from `novo_parts::circuit_library()` by name (`Circuits`), one hoist page per unit. Open points of the circuits show as warnings (UnconnectedPin) on purpose: they are where the supply is connected.
- Pump station and motor control centre templates: `template_plants.rs` (`Plant`, `PlantOptions`, `build_plant_template_on`), window `template_plants_window.rs`, `wb.plant_new`, commands `file.new_pump`, `file.new_mcc`. Reuse the crane `Circuits` helper. `project.kind` ("crane", "pump", "mcc") hides the crane-only cover rows and Project information fields (`front_keys::shows_crane_fields`). A missing library circuit is now an error, not a panic.
- Ideas not done: a frequency-drive pump feeder circuit (the library has none without a brake), level control page and frequency-drive pump feeder (waiting for circuits from the library thread).
- MCC tree now has a `=MCC+FLD+Mn` motor node per feeder.
