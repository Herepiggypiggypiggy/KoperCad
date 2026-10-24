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

Views (2026-10-22): `crates/novo-app/src/app/emc_adapter.rs` turns `novo_emc::Picture` into the view3d `Picture`. It takes a `drawn` set of wire ids so a view never shows a finding about a wire it does not draw. Heights and bonds (2026-10-24): `Device.height_mm` (the device's depth property) reaches `Zone.height_mm` and the node height (default 60 mm when not stated). `Picture.bonds` holds one case-to-plate bond per drive/filter whose plate finish or `emc.bonded` is stated (Good bare/galvanised or stated bonded, Weak painted, Bad stated not bonded; nothing stated = no bond). Same facts as EMC-11, rule-based. Known limits: route points sit at z = 0, plate-to-enclosure and PE bar links are not modelled, fix buttons do not apply from the card.
