# EMC visualisation: handoff

Branch `claude/project-thread-bt64vs`. #1388 and #1391 merged; open: the 2D plate mirror PR. First PR "EMC view in the 3D cabinet" (see PROGRESS.md for the head).
Files: `crates/novo-view3d/src/emc/` (picture, field, geom, overlays, style), `crates/novo-theme/src/emc.rs`, `crates/novo-app/src/app/{emc_view,emc_stub,emc_demo}.rs`, QA `qa/scenarios/96-3d-emc.qa`, design `docs/design/emc-view.md`.
Commands: `view.emc`, `file.example_emc`.

Resume: `cargo test -p novo-view3d -p novo-theme -p novo-workflow -p novo-app`; `scripts/qa.sh --only emc`. Evidence in /mnt/project-files/emc/evidence/.
Traps: IEC letter codes are ambiguous (VFD and PSU are T, PLC is K), so classes come from the symbol key and pin names. Panel: a scope per chip cannot wrap (no add_enabled_ui in a wrapped row). Windows without `.height()` default to 420 px.
Rule lead: requests in /mnt/project-files/emc/requests-to-visualisation.md are all met.

2D plate: `emc_plate.rs` draws the Picture over the plate; the picture comes from `EmcState::sync_plate` (routes of the Cabinet overlay plus plate placements). Commands: `cabinet.emc`. QA `qa/scenarios/97-cabinet-emc.qa`, evidence /mnt/project-files/emc/evidence-plate/.
