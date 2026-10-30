# Controller machine-module bindings (L5b): handoff

Branch `claude/controller-machine-bindings`, one PR from main. Status: see PROGRESS.md for the PR number and head.

## What exists
- Target text `module@port` (`hoist/-M1@up`, `travel/-M2@permit`); module = `kind/<motor tag>` (`bridge/` or `trolley/` for the
  axes the crane picture uses). Parsing, catalog and checks: `crates/novo-controller/src/{machine,machine_check}.rs`.
- Ports and the circuit wrapper: `crates/novo-machine/src/commands.rs`. A bound port replaces the drawn input; unbound ports keep
  the drawn circuit; `permit` and `deadman` gate a module only when bound. Virtual tags `~cmd:` (inputs, off by default) and
  `~on:` (main contactor and safety channels, on by default) fill empty binding slots; `CommandedCircuit` answers for them.
- Host: `crates/novo-app/src/app/controller_machine.rs` (catalog, wirings), `controller_live.rs` (resolve, `machine_commands`),
  `hoist_run::advance` and `drive_run::advance` (wrap the circuit once per frame), `controller_card.rs` (picker, block).
- Docs: `docs/design/controller-live.md`, section "Machine ports".

## Resume
```
export CARGO_TARGET_DIR=/tmp/kc-target-machine CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p novo-controller -p novo-machine
cargo test -p novo-app --lib controller_
cargo test -p novo-tests --test radio_machine_fixture      # KOPERCAD_WRITE_FIXTURES=1 rewrites qa/fixtures/radio-machine.novo
scripts/qa.sh --only controller-machine --size 1366x768
```

## Traps and decisions
- The travel loop file is owned by another thread (brake, inertia, wind): only its public fields are used (`binding.inputs`,
  `binding.mains`). Do not edit its physics from here.
- The loops read tags from their binding; `Wiring::*` fills empty slots with virtual tags only for bound ports and does it every
  frame (idempotent). Do not read a virtual tag from the real circuit: the wrapper keeps them out of `set_contact`.
- A pair of winches is commanded through winch A's binding (the pair loop reads only A's up, down, fast).
- Tests that act on the controller need one `LiveView::solved` frame first (it reads the project's controllers).
- No new live card was added (the existing Controller card grew a block), so no lesson anchor was needed.
- Disk: the shared allowance fills up with several target dirs; `scripts/free-disk.sh`, `CARGO_INCREMENTAL=0`.
