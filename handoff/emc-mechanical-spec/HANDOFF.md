# Handoff: EMC and mechanical on the enclosure spec

- `crates/novo-layout/src/spec_fit.rs`: `fit(spec, typed)` gives the `Enclosure` (inside size, plate setback, door inner depth) from the record. Hinge, angle and free room still come from the typed door form. Inside size comes from outside size minus the wall; depth is measured from the inside of the back wall.
- `crates/novo-checks/src/door.rs`: when a structure has a record, `layout.enclosure` is replaced by the record's room before the report; messages say `(enclosure data: <source>)`. New problem `EnclosureDataMissing`. A too-deep device is a warning, not an error, when the enclosure numbers are program defaults.
- `crates/novo-emc`: `Plate.shell` is read from the record (`project::shell_of`); `rules::shell` is EMC-22, `lowest_cavity_hz` is the closed form.
- Tests that change the spec and see the result change: `novo-layout spec_fit`, `novo-checks tests/door_checks.rs` (`the_chosen_enclosure_decides_the_depth_check`), `novo-emc tests/engine.rs` and `tests/project.rs`.
- QA: fixtures `qa/fixtures/enclosure-spec-wide.novo` and `-small.novo` (synthetic numbers, regenerate with `cargo test -p novo-app --lib write_the_enclosure_spec_fixtures -- --ignored`), scenarios 18900 and 18901.
- Where the user finds it: Check > Problems (door and depth problems), Check > EMC check (EMC-22).
- Not done: weight against load rating, wall material and seams (the record has none of them).
