# EMC rule engine: handoff

State 2026-10-22: engine written and tested in `crates/novo-emc`; wired into `crates/novo-checks` (`emc.rs`, `ProblemKind::Emc`, EMC family in `standards/design.rs`) and `crates/novo-app` (`app/emc.rs`, Action::EmcCheck, command `check.emc`). Branch `claude/project-thread-adh8ja`; registration PR #1381, engine PR opens from main once #1381 is merged (no stacked PRs).

Resume: `cargo test -p novo-emc -p novo-checks`, then `cargo test -p novo-app emc`. Read `docs/EMC.md` section 4 (levels) and 5 (catalogue) first.

How it fits
- `novo_emc::project::input_of(project, netlist)` builds the plain `Input` (runs with classes and routed paths, ducts, devices, plates, params); `novo_emc::analyse(&Input)` returns the `Report`; `picture(&input, &report)` arranges it for drawing. `project::apply(tx, &Fix)` is the only writer.
- Facts the model cannot hold yet are properties (table at the top of `project.rs`). Turn the check on with project property `emc.check`.
- Scoring is explicit: points per finding (see `rules.rs`), the sum is the score, every point has its reason.

Traps
- Words: never "pass", "compliant", "certified"; use "no finding" and "Design aid, not a test result." Rules carry `verify: true` (clause numbers unverified).
- Estimates only inside `BAND_HZ` (to 30 MHz); above it the rule says not evaluated.
- Colours in UI come from `crate::theme::ui_palette()`.
- A motor wire not in a cable is an EMC-04 finding by design: a bare test wire in a fixture needs a shielded cable first.

People: EMC lead thread and visualisation thread share `handoff/emc/`; the roster with session ids is `coordination/threads.md`.
