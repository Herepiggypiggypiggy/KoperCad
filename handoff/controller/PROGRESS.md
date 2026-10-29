2026-10-28 09:41 UTC C0: registered novo-controller with skeleton, test, CLAUDE.md row.
2026-10-28 10:26 UTC C1: engine, validator, schema, instances, bindings, sequences; tests written first against real two-step/E-stop/deadman behaviour; wasm32 build checked.
2026-10-28 11:05 UTC C1: #1558 merged (84bda25). Follow-up PR from main with R2/R1 findings: restore settles, `combine = "and"` for NC chains plus shared-chain and same-controller checks, replay overflow and sample cap, id check, clock catch-up, requires-cycle warning, Result-returning to_toml.
2026-10-28 11:29 UTC C1: #1560 Windows CI failed (CRLF checkout broke a multi-line test replace); test now replaces the single line, verified with a CRLF fixture.

2026-10-28 C4: import engine and wizard built, 27 tests in novo-controller plus 5 in novo-app; output validated by hand against C1 validator (0 issues). Open: PR, wire to `novo_controller::Profile` after C1 merges.
2026-10-28 10:30 UTC C1: engine, validator, schema, instances, bindings, sequences; tests written first against real two-step/E-stop/deadman behaviour; wasm32 build checked.

2026-10-28 C4 follow-up: preview counts say "1 control", the "to check" list comes first in the preview (visible at 1366x768), warning texts use quotes instead of backticks.

2026-10-28 C4: preview shows "N rows: scroll the preview below to see every row and note" and an always-visible scrollbar (R3 note from #1615).

2026-10-29 C4: `ProposedProfile::to_profile` builds a `novo_controller::Profile` directly (a test checks it equals the profile read back from the TOML); the wizard shows the engine validator's issues in the preview and refuses to save on an error. Bare direction words (RIGHT, FORWARD, UP) now use the crane convention (trolley, bridge, hoist) with an Info note, and a row that has only a step (for example "H SNEL") says so. Not done: tuning on the user's real I/O pages, because the device folders were not reachable from this thread; needs his files or a thread with his device approved.

2026-10-29 C4 tuning on real page patterns (Telecrane-style receiver pages with shared fast pins, Dutch labels, 48Vac/0Vac supply rows, compass words on the 10-motion receiver, Mains-1/2, bulleted output lists). Fixes: pair abbreviations, compass combos, mains, supply/neutral rows, bullets. Tests use invented tables only (`tests/import.rs`, last six tests). Real-page facts are in plans/crane-setup-facts.md in the project folder, not in this repository.
