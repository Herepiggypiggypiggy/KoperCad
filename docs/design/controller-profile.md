# Controller profile and engine (lane C1)

Crate `novo-controller`. Plan: `plans/controller-live-view.md` (project files). The engine simulates the wiring and
logic of an operator controller (radio, pendant, joystick, PLC test panel). It is not a radio protocol, receiver
firmware or compliance model. Profiles shipped with the program are generic; vendor profiles are made by the user.

Where the user finds it: not yet in the app. This PR is the engine only. The panel, instance list and binding table
come with lane C3 (live view card, Shift+L) and the add-on manager (C0b); the engine has no UI dependency by design.

## Profile file (`*.controller.toml`)

Schema: `crates/novo-controller/schema/controller-profile.schema.json` (structure; a test keeps it in step with the
types). The validator (`novo_controller::validate`) checks what a schema cannot and returns every problem at once.
Example: `crates/novo-controller/tests/data/generic-radio-3x2.controller.toml`.

| Section | Meaning |
| --- | --- |
| `[controller]` | `id`, `name`, `kind` (radio, pendant, joystick, plc-panel, custom), `supply`, `common`, `step2_includes_step1`, `shared`, `generic`, `gate` |
| `[effects]` | `self_test_ms`: every signal stays low this long after power-up (0 = off) |
| `[[control]]` | `id`, `kind`, `positions` (rest first, or `rest`), `spring`, `needs_link`, `requires`, `drops_when` |
| `[[signal]]` | `name`, `control`, `kind` (contact, voltage, analog), `when` or `group` + `step` or `levels`, `requires`, `ungated`, `needs_link`, `pin` |
| `[[interlock]]` | `between = [controls]`, `mode` = `first_wins` or `both_off` |

Control kinds and defaults: `button` (off/on, spring), `latch` (off/on, press toggles), `rocker2`/`rocker3` (positions
`<group><step>`, spring), `axis` (detents, spring), `selector` and `key` (stay), `estop` (released/pressed, leaves
`pressed` only by `reset`), `deadman` (released/held, spring).

Semantics, all covered by `tests/engine.rs`:
- Step contacts: a signal with `group = "up"`, `step = 1` is the slow contact. With `step2_includes_step1` (default) the
  slow contact stays closed at step 2; `false` drops it. Three-step rockers extend the same rule.
- `gate` conditions (for example `start=on`) apply to every signal except `ungated` ones (the main contact, the E-stop chain).
- `requires` and `drops_when` on a latch model the receiver: the latch cannot be set unless the conditions hold, drops
  when they stop holding, and stays dropped (E-stop out does not restart main).
- `needs_link` on a control: drops on link loss or power-off. On a signal (default true for `radio`): low while the link is down.
- Selector mapping is written as signals with `requires = ["sel=position"]`.
- E-stop is an NC contact: the chain signal is high while `released`.

## Instances, bindings, sequences

- `ControllerSet`: several instances per project (id, label, colour, target group, own state). The same profile may back
  any number of instances. `to_records`/`from_records` plus `ProjectControllers` serialize with serde (TOML or JSON).
- `BindingTable`: bindings keyed `controller/signal` to a pin, symbol, PLC address or module-instance path. `check` finds
  unbound signals, bindings to missing signals, duplicates and two controllers on one target unless `shared`. `resolve`
  gives each target its level (largest magnitude wins), `owners` says who drives a target now.
- `Sequence` (`.sequence.toml`): timed actions in integer milliseconds. `ControllerSet::start_recording`/`stop_recording`
  record; `Sequence::replay` and `Player::advance_to` replay with no clock or randomness, so two replays give equal traces.

## Packaging

std only, `BTreeMap` everywhere, no files, clock, threads or globals: it builds for `wasm32-unknown-unknown`
(`cargo build -p novo-controller --target wasm32-unknown-unknown`) so it can be the add-on `engine.wasm`. The host JSON
API for the wasm module is lane C0b's job and is not part of this PR.
