# Terminal strips and bulk location: handoff

Branch `claude/project-thread-8hpaox` in kopercad. Changes:
- `novo-terminals`: `grouping.rs` (StripClass, Grouping, `grouping`, `set_grouping`), `suggest.rs` (`Suggestion.class`, `suggest_terminals_with`). Class comes from the potential on the wire's net: Line/Neutral, a DC rail above 120 V and a Signal above 50 V are power, DC rails up to 120 V (or without voltage) and Signal up to 50 V are control, Signal without voltage is signals, PE/PEN is earth, no potential is "unassigned" and stays on the first strip (so old projects behave as before until potentials are assigned).
- `novo-parts` `sheet.rs`: row `location` (devices and cables; wires in a box selection are ignored); refuses a clash with an unselected device or between two selected ones.
- `novo-app`: Devices tab "Set location of N selected" field, Terminals tab strip grouping chooser beside "Accept N proposed", registered the lesson anchor `drawer.terminals.accept` (was missing: debug builds panicked when the Terminals drawer had proposals).
- Tests: `crates/novo-terminals/tests/terminals.rs` (grouping), `crates/novo-parts/tests/parts.rs` (set location), `crates/novo-app/src/app/strip_grouping_tests.rs`.
- QA: `scripts/qa.sh run qa/scenarios/69-set-location-and-strips.qa --size 1366x768`, `.../70-set-location-and-strips-1920.qa --size 1920x1080`. Fixture rewrite: `cargo test -p novo-app write_strip_grouping_fixture -- --ignored`.

Traps: no novo-core API change (the setting lives in project info properties). Needs libxkbcommon-x11-0 for QA.
