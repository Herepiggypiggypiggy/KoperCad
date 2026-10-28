# EMC filter fixes: handoff

Branch `claude/project-thread-i5iv6i` from main 78a1dab. See ROADMAP.md and PROGRESS.md.

- `novo-emc`: `Device.wired` (new field) is set in `project::input_of` (device has >= 2 wired pins). `rules::drive_filters` treats an unwired filter as a to-do (no fix, finding stays).
- `Fix::AddShield` also sets unstated `emc.shield_a/b` to `360`.
- `novo-canvas`: Interruption symbols now get a tag slot, so their names avoid each other.
- `novo-app`: `Panel::closable()` plus `panel::take_closed`; used by the schematic and cabinet inspectors. `count_words::wires` for singular wire counts.
- QA: `qa/scenarios/1581-emc-filter-needs-wiring.qa`, 156 and 157 updated.
