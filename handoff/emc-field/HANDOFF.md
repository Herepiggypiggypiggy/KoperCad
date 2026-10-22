# EMC magnetic field estimate: handoff

State 2026-10-22: built on branch `claude/project-thread-2yotei`; PR link and head SHA in PROGRESS.md.

Resume: `cargo test -p novo-emc` (field.rs unit tests, tests/field.rs, tests/project.rs), then `cargo test -p novo-app emc` and `cargo test -p novo-tests --test emc_field_fixture`. QA: `scripts/qa.sh --only emc-field`.

How it fits
- `novo_emc::analyse_with_field(&Input, Some(&FieldSetup))` runs everything plus EMC-19 and EMC-20; `analyse(&Input)` is unchanged (no currents: both rules report "not evaluated"). `project::field_setup_of(project, netlist, input)` builds the currents and settings; `project::analyse_with` and `picture_of` use it, so Check > Issues sees the findings.
- `Report.field` carries the tables (currents, devices, victims) for the window; `field::map(input, setup, plate, size)` makes the grid; `app/emc_adapter.rs::field_of` turns it into the view3d `Field` (0..1 log ramp, `ends` for the legend).
- `Input` and `Params` were not touched on purpose, so hand-built inputs in other tests keep compiling.
- Touched shared files: `rules.rs` (two RULES entries, `analyse` split, status/assumptions merge), `report.rs` (`Report.field`, one NOT_CHECKED line), `novo-view3d/src/emc/picture.rs` (`Field.ends`, `Kind::Field`), `emc_view.rs` (legend and chip text), `emc.rs` (window section).

Traps
- Wires on one duct line are laid one diameter apart (`laid_paths`), otherwise balanced phases cancel exactly.
- The 50 Hz estimate is free space unless `emc.plate_image` or 1 kHz and up; the step estimate always uses the plane.
- Words: "estimate", "model", "not a measurement"; never "pass" or "compliant".
- Colours from `crate::theme::ui_palette()`.
- Hot spot list shows 6 rows: a seventh finding is reached by scrolling.

People: EMC lead and visualisation thread (`handoff/emc/`), rule engine thread (`handoff/emc-engine/`); roster `coordination/threads.md`.
