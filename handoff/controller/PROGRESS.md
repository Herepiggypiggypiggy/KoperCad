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
2026-10-29 C2 follow-up (R1 finding on #1615): new binding check `MixedOrAnd` warns when one target mixes an `and` signal (E-stop chain) with an ordinary one, because the ordinary signal, while high, overrides an open chain. Documented in `resolve` and in docs/design/controller-live.md; test in novo-controller/tests/project.rs. The card shows it with the other findings.
