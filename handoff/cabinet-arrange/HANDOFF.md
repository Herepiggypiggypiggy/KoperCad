# Cabinet arrange tools: handoff

Updated 2026-11-02.

## State
- Branch `claude/project-thread-5tafis` on `kopercad`. First PR adds the engine, commands,
  preview and the Arrange card of the Cabinet rail. See PROGRESS.md for the head SHA.
- Engine: `crates/novo-layout/src/arrange.rs` (tests in `crates/novo-layout/tests/arrange.rs`).
- App glue: `crates/novo-app/src/app/cabinet_arrange.rs` (+ `_tests.rs`), `rail_cabinet.rs`
  (`rail.arrange` entry and flyout), `cabinet_menu.rs` rows, `workbench.rs` command arm,
  `cabinet.rs` (`Ask::Arrange`, banner text, accept label).
- Seven commands: `cabinet.arrange_all` (Auto-reconfigure, chains the others in one plan) and
  `cabinet.arrange_onto_rail|rails|ducts|group|tight|even`.

## How it works
Tools read a `CabinetLayout` and return a `Plan` with `moves` (devices already on the plate),
new `rails` and `ducts`, and `notes` for what was left alone. `CabinetLayout::accept` shows it,
`model::apply_plan` stores it (moves go through `PlaceDevice`, one transaction).
A tool never moves a device onto another, a duct, or off the plate; gaps round up to whole mm.

## Resume
```
cargo test -p novo-layout --test arrange
cargo test -p novo-app --lib cabinet_arrange
```

## Traps
- A new command must be added in `commands.rs`, `homes.rs`, `menus.rs`, `language.rs`,
  `presentation.rs`, `workbench.rs` (APP_COMMANDS and an explicit match arm; the tips test
  scans for string arms, a guard arm is not seen) and the drawn list in `rail_workspaces_tests.rs`.
- Disk fills: `scripts/free-disk.sh`, `CARGO_INCREMENTAL=0`.
- Not done: QA screenshot scenario (needs the headless harness), auto-reconfigure.
