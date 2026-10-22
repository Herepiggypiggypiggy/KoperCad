# Vents and airflow in 3D heat: handoff

Thread: "Vents and airflow in 3D heat" (cmsg_01TV2QyH4AYw9X2aTmgUsuhCWNkh8ptbGCgzzcD9CRG8Xk).
Branch `claude/project-thread-827buu` in kopercad. PR: see PROGRESS.md for number and head.

Where the user finds it: 3D view, Heat button in the toolbar: the Vents and fans panel opens on the
left, the Heat panel on the right. Add opening lists sensible places; click a row to edit it, or click
an opening's label in the cabinet.

Files: `crates/novo-app/src/app/heat_settings.rs` (model, presets, undo), `heat_openings.rs`
(3D geometry, labels, flow read from the result, inlet seeds), `view3d_vents.rs` (panel),
`view3d.rs` (wiring: `vent_*` fields, after the pins block), `heat_3d.rs` (`flow_from`,
`speed_rgb`, `stream_lines` with arrowheads), `heat_legend.rs` (m/s bar).
Tests: `heat_settings`, `heat_openings`, `view3d_vents_tests`; QA `qa/scenarios/75-3d-heat-vents.qa`.

Honesty: flows are model values (label says "(model)"); filter fan = the fan with datasheet values
with filter fitted; planning size fan 150 mm, 100 m3/h, 60 Pa is labelled as such.
ENCLOSURE_VALIDATED unchanged (false).

Traps: a grid cell does not wrap (a note in the editor grid stretched the panel to 640 points);
the panel needs height 560 so the rows stay visible with a fan open at 1366x768.
QA needs `libxkbcommon-x11-0` etc. (apt-get update first).
