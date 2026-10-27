# Wire-it handoff
Branch claude/project-thread-i16fyt (kopercad). Code: crates/novo-reports/src/wireit.rs (engine), crates/novo-pdf/src/wireit.rs
(sheet), crates/novo-app/src/app/view3d_wire_it.rs (strip, overlays, state), wire_it_export.rs (PDF command), edits in
view3d.rs, view3d_controls.rs, workbench.rs, lengths.rs and wirecut.rs (route per job, plans cached with the jobs),
commands, menus and Dutch words in novo-workflow.
Tests: crates/novo-reports/tests/wireit.rs, crates/novo-pdf/tests/wireit.rs, crates/novo-app/src/app/view3d_wire_it_tests.rs.
QA: scenario 520 (qa/scenarios/520-wire-it.qa), pictures in /mnt/project-files/qa/wire-it/.
Resume: `export CARGO_TARGET_DIR=/tmp/target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`;
`WIREIT_DUMP=/tmp/w.pdf cargo test -p novo-pdf --test wireit a_dump` writes a sample sheet.
Traps: the lit path floats in front of the ducts because overlays are depth tested; scene wire ids of Wire-it are
`1<<48 + step number`, not project wire ids (the Run card and hover have no facts for them, on purpose); progress keys
are pins, not wires; the plan is kept across the app's own ticks (State::ticked) to avoid routing again.
Decisions taken: pass order earth, power, 24 V, signals; short before long inside a track; dense landscape list instead
of half-page cards. All reversible.
