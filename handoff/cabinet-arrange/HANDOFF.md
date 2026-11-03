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

## PR 2: Autogroup by wiring and heat (branch `claude/project-thread-nyj5hs`)
- Pushed, NO PR yet: it carries PR 1's commits, so it waits for #1816 to merge. Then:
  `git fetch origin main && git rebase --onto origin/main <last PR 1 commit> claude/project-thread-nyj5hs`
  (PR 1's last commit is `b4ce9eb`; only `9ccb563` and later are PR 2), re-run the tests below, open the PR from main.
- Engine `crates/novo-layout/src/regroup.rs` (tests `tests/regroup.rs`), app glue in `cabinet_arrange.rs`
  (`wiring_links`, `regroup_plan`). Commands `cabinet.arrange_group_wiring|heat|both`, QA `qa/scenarios/1721-*.qa`
  (written, NOT run: the authoring box has no openbox/vulkan/libxkbcommon-x11, so no screenshots yet).
- Resume: `cargo test -p novo-layout --test regroup`, `cargo test -p novo-app --lib cabinet_arrange`.
- Honest limits: wire length is a straight-line estimate, heat is the planning coupling score, only devices on
  horizontal rails move, rails and ducts are not changed, a net of more than 12 devices is ignored.
