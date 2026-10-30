2026-10-28 09:41 UTC C0: registered novo-controller with skeleton, test, CLAUDE.md row.
2026-10-28 10:26 UTC C1: engine, validator, schema, instances, bindings, sequences; tests written first against real two-step/E-stop/deadman behaviour; wasm32 build checked.
2026-10-28 11:05 UTC C1: #1558 merged (84bda25). Follow-up PR from main with R2/R1 findings: restore settles, `combine = "and"` for NC chains plus shared-chain and same-controller checks, replay overflow and sample cap, id check, clock catch-up, requires-cycle warning, Result-returning to_toml.
2026-10-28 11:29 UTC C1: #1560 Windows CI failed (CRLF checkout broke a multi-line test replace); test now replaces the single line, verified with a CRLF fixture.

2026-10-28 C4: import engine and wizard built, 27 tests in novo-controller plus 5 in novo-app; output validated by hand against C1 validator (0 issues). Open: PR, wire to `novo_controller::Profile` after C1 merges.
2026-10-28 10:30 UTC C1: engine, validator, schema, instances, bindings, sequences; tests written first against real two-step/E-stop/deadman behaviour; wasm32 build checked.

2026-10-28 C4 follow-up: preview counts say "1 control", the "to check" list comes first in the preview (visible at 1366x768), warning texts use quotes instead of backticks.
2026-10-28 C2: controller device in the live circuit. `novo_sim::pins` (pin to pole), `Bench::override_input`, `ControllerLive` (state, per-frame drive, storage, checks, auto-bind), Controller card, `sim.controller`, receiver symbols `iec/controllers/*`, fixture and tests (novo-app 12, novo-sim 8, novo-symbols 2, novo-workflow 1, novo-tests 3), QA 1581/1581b/1582 at both sizes. Found by QA: the card crashed in debug builds until its lesson anchor `live.controller` was registered.
2026-10-28 C2: merged main (hoist run, L5a) into #1615; QA 1583 shows the radio lifting the hook through the drawn circuit (the controller drives the loop through LiveView::solved, no extra code).

2026-10-28 C4: preview shows "N rows: scroll the preview below to see every row and note" and an always-visible scrollbar (R3 note from #1615).

2026-10-29 C4: `ProposedProfile::to_profile` builds a `novo_controller::Profile` directly (a test checks it equals the profile read back from the TOML); the wizard shows the engine validator's issues in the preview and refuses to save on an error. Bare direction words (RIGHT, FORWARD, UP) now use the crane convention (trolley, bridge, hoist) with an Info note, and a row that has only a step (for example "H SNEL") says so. Not done: tuning on the user's real I/O pages, because the device folders were not reachable from this thread; needs his files or a thread with his device approved.

2026-10-29 C4 tuning on real page patterns (receiver pages with shared fast pins, Dutch labels, 48Vac/0Vac supply rows, compass words on the 10-motion receiver, Mains-1/2, bulleted output lists). Fixes: pair abbreviations, compass combos, mains, supply/neutral rows, bullets. Tests use invented tables only (`tests/import.rs`, last six tests). Real-page facts are in plans/crane-setup-facts.md in the project folder, not in this repository.
2026-10-29 C2 follow-up (R1 finding on #1615): new binding check `MixedOrAnd` warns when one target mixes an `and` signal (E-stop chain) with an ordinary one, because the ordinary signal, while high, overrides an open chain. Documented in `resolve` and in docs/design/controller-live.md; test in novo-controller/tests/project.rs. The card shows it with the other findings.

## 2026-10-30 Several controller instances and panels (branch claude/project-thread-uvlaiu)
Instance tabs with colour and active ring, rename/colour/group, duplicate, pop-out floating panels (several at once),
shared-target lines with owner and conflict, `BindingTable::ownership`, `ControllerSet::is_active`. Tests in
`novo-controller/tests/project.rs` and `controller_live_tests.rs`. Open: OS-level windows, saving which panels are out,
signal monitor filter by instance, binding table controller column, Machine workspace module owners.
