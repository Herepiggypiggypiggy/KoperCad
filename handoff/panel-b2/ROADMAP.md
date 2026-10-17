# PANEL-B2 roadmap: door swing, openings, per-surface place-all

1. Done (slice 1, #752): door swing sweep, `swing.rs`.
2. Slice 2 (#773): wiring openings, `openings.rs`: `opening.<name>` structure property, keep-out rule `opening.keepout` in the layout report and device rows, `best_passage` for routing.
3. Slice 3 (#782): surface rules, `surface_rules.rs`, Place all keeps door devices off the plate plan.
4. Done (#876): `door_devices` and `off_plate` from the `surface` property.
5. Done (#889): openings in the drill table. Done (#893, #900): Place all plans the door, with a preview. Done (#896): routing through the best opening.
6. Next: drill-table rows for openings on side walls and extra plates; plate-to-door gap in wire lengths; `surface_names` should call `model::extra_surfaces`.
