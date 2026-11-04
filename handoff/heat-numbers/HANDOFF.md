# Heat numbers: probes, line plot, volume figures

Built 2026-11-04 from research/paraview/SUMMARY.md items 2, 3 and 4.

- Engine: `crates/novo-heatview/src/probes.rs` (`line_profile`, `air_numbers`, `air_histogram`, CSV). Tested on the linear field.
- UI: `crates/novo-app/src/app/heat_numbers.rs` and `heat_probe.rs` (pins, line state, overlays), wired in `view3d.rs`.
- Where the user finds it: Cabinet > 3D > Heat strip. Probe on, then Pin and Line in the probe row; Numbers menu next to Hot spots.
- Not built: temperature-against-time curves for pinned probes in a heat-up run (the 3D view holds one snapshot; needs the time frames from heat_time), flow-weighted outlet temperature, report/PDF export of the numbers.
- Honesty: model values from the solver, labelled in the "?" texts.
