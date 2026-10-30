# Controller machine-module bindings (lane L5b): roadmap

Scope: a virtual controller signal can be bound to a port of a module instance of the machine model, so the controller
drives the hoist, travel, drive hoist and winch runs. Plans: `plans/controller-live-view.md` (sections 0, 2, 6 lane C2),
`plans/drive-crane-simulation.md` (L5b). Design text: `docs/design/controller-live.md`, section "Machine ports".

## Done in the PR (branch `claude/controller-machine-bindings`)
- `novo-controller::machine`: `MachinePath` (`module@port`: parse, validate, display), `MachineCatalog` (plain data the host builds),
  `PortKind` (command, chain, enable). `Target::machine`, `Target::machine_path`, `BindingTable::check_with` /
  `check_machine` (syntax, unknown module, no port, unknown port, wrong signal kind for the port, two owners of a module, two
  spellings of a port) and `resolve_machine`. Still wasm-clean (serde and toml only). Serde: the target stays
  `Target::ModulePath(String)`, so `ProjectControllers` saves it with no format change.
- `novo-machine::commands`: `Port`, `Commands`, `Wiring` (hoist, travel, drive hoist, closed winch, winch pair),
  `CommandedCircuit` (a `CircuitPort` wrapper). The loops and the travel physics are untouched.
- `novo-app`: `controller_machine.rs` (catalog from the project, wirings of the running loops), `ControllerLive` resolves machine
  targets and exposes `machine_commands`, `machine_drivers`, `catalog`; `hoist_run::advance` and `drive_run::advance` wrap the
  circuit; the Controller card has a Machine ports picker and a Machine modules block; the run windows print which signal drives a module.
- Tests: novo-controller `tests/machine.rs`, novo-machine `commands.rs` and `commands_tests.rs` (fixed steps: press, release,
  E-stop, dead-man, determinism), novo-app `controller_machine_tests.rs`, novo-tests `radio_machine_fixture.rs`. Fixture
  `qa/fixtures/radio-machine.novo`. QA `qa/scenarios/17300-controller-machine-ports.qa`.

## Next
- Machine workspace panel (there is no Machine workspace yet; the run windows and the card show the drivers).
- Brake release for drive hoists and winches is the drive's own logic and is not a port; revisit with the drive validation lane (L7).
- A `target_group` check: warn when an instance with a target group (`runway1/bridge1`) is bound outside it (needs the module hierarchy).
- Analog ports (a joystick axis as a speed reference) once the drive models take a reference from outside.
- Travel brake, inertia and wind are owned by another thread (travel drive); nothing here depends on them.

## Open questions
- Module paths use `kind/<motor tag>`; a project with a runway/bridge/crab tree would want real hierarchical ids.
