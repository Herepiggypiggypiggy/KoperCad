# Autopilot handoff

Branch `claude/project-thread-md0z6t` (repo kopercad). PR: see PROGRESS.md for the number and head SHA.

## State
Engine, window, command, meter and tests are done for sizes, colours and wire numbers. Cable types, routes, ferrule rows and the datasheet-to-part flow are not started (ROADMAP.md).

## Where things are
- `crates/novo-reports/src/autopilot.rs`: `plan`, `Autopilot::apply`, `ready`, unit tests against IEC 60364-5-52 Annex B method C values, derating and voltage-drop cases.
- `crates/novo-checks/src/calc_basis.rs`: `wire_basis` (what each wire's sizing rests on). Shares defaults with the Calculations window.
- `crates/novo-app/src/app/autopilot.rs` (+ `autopilot_tests.rs`): window, plan building, apply as one undo step (`edit_project`).
- Command `wires.autopilot` in `novo-workflow` (commands.rs, language.rs), dispatch in `workbench.rs`.
- Bench: `bench.rs` shows the meter and the Autopilot button in "Before you print".
- QA: `qa/scenarios/08-autopilot.qa`.

## Resume
```
export CARGO_TARGET_DIR=...   # one shared dir; scripts/free-disk.sh when a write fails
cargo test -p novo-reports autopilot
cargo test -p novo-app autopilot
scripts/qa.sh --only autopilot
```

## Decisions and traps
- Honesty: ampacity tables are planning values; every size row carries its basis. Rows below 1.5 mm2 use low planning caps, not a table.
- A plan refuses to apply when the drawing changed (revision check); the window re-plans on the next frame.
- The current of an unrated wire comes from the nearest rated devices (K1 AC-3 rating, overload setting, F3). The highest is used, stated in the reason.
- Circuit kind comes from symbol keys (`/motors/`, `main_`, `_3p`, overload relay = power; everything else on the net = control). New symbol families may need a line in `is_power_symbol`.
- Meter counts number, size, colour and routed length per wire, plus a mounting plate; estimates count as not done.
