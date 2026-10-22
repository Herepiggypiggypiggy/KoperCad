# Cooling advice and vent sweep: handoff

Thread "Cooling advice and vent sweep" (cmsg_01TV2QyH4AYw9X2aTmgUsuhCDtntvz56JxktCBRtZ9Q9Hz).
Branch `claude/project-thread-phf292` in kopercad. PR: see PROGRESS.md.

Where the user finds it: Cabinet or 3D view, Heat strip, button **Cooling advice**; Ctrl+K "Cooling
advice and vent sweep"; Cabinet plate right-click "Cooling advice…".

Files: `crates/novo-thermal/src/sweep.rs` (engine), `crates/novo-thermal/tests/sweep.rs`,
`crates/novo-app/src/app/heat_sweep.rs` (window, conversion to `VentSpec`), `heat_sweep_tests.rs`,
wiring in `heat.rs` (button, `advice` field), `cabinet.rs`, `view3d.rs`, `workbench.rs`,
`cabinet_menu.rs`, `novo-workflow` (command + Dutch words). QA `qa/scenarios/76-cooling-advice.qa`,
fixture `qa/fixtures/cabinet-cooling-advice.novo` (same file as the Cooler layout fixture).

Honesty: every number is the 2D model with planning values (`ENCLOSURE_VALIDATED` stays false). Fan
curves are straight lines from the free-air flow to 60 Pa. The window says so and says when a gain is
under 1 K. The sweep does not move devices (that is Cooler layout, #1365).

Cost: about 130 runs; 12 s in a release build on 4 cores for a 3-device cabinet, longer for 100+ devices
(runs on a worker thread, Stop keeps the best so far). Tests use `Settings::quick()`.

Merge notes: touches the same lines as #1365 (button in `simulate_controls_with`, `HeatState` fields,
`workbench.rs` command lists) and #1342 (heat.rs); both resolve by keeping both sides.
