# Heat model and cooling: handoff

Branch `claude/project-thread-kcg14f` (repo kopercad). PR 1: design doc, `novo-layout::thermal`
engine and bridge, tests (`crates/novo-layout/tests/thermal.rs`), this folder.

State: engine done and tested; no UI yet. Resume: read `docs/design/thermal.md`, then do
ROADMAP item 1 (overlay in `novo-canvas/src/cabinet.rs`, canvas crate does not depend on novo-layout,
so pass plain `HeatCellView`s like the other views).

Decisions: mean interior temperature only (no IEC 60890 curves, tables unavailable); unknown loss is
never 0, result is `Incomplete`; load factor is linear; enclosure inputs are Structure properties.
Reviewer: message the main reviewer with the head SHA after every push.
