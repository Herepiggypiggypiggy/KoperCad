# Protection coordination plot

Scope: Protection > Curves shows device trip bands, the damage curve of the cable each device protects, and the start of each motor behind it, on one log-log plot, with a verdict per pair.

Done
- Engine `novo-checks/src/protection/plot.rs`: `CoordinationPlot::read(project, study, single_line)`.
- Drawing and checks list in `novo-app/src/app/protection.rs` (curves tab), "?" help carries the model-or-data wording.

Next
- Maker curves (point lists from the user's own files) instead of the standard bands.
- Let-through energy verdict below 0.1 s (today the existing coordination findings cover it).
- Motor start from the sim motor model; inrush peak; several motors on one device summed.
