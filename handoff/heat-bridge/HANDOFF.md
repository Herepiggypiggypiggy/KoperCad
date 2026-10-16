# HEAT-H handoff

Code: `crates/novo-thermal/src/bridge.rs`, tests `crates/novo-thermal/tests/bridge.rs`, part keys `LOAD_FACTOR`, `MAX_AMBIENT_C`, `MAX_SURFACE_C` in `novo-parts` `keys`. Command stub `heat.simulate` in `novo-workflow` (shows "not in the app yet" until CFD-F handles it).

Rules: device property overrides part; negative/NaN values count as missing; loss of several parts is summed; unknown loss is placed as 0 W and listed, never guessed. Plate is centred in the interior. Depth key is `depth_mm` (not `height_mm`, which is the footprint height in the plate plane).

#406 is closed; its loss-density map is not part of this lane (engine only, per spawn table).
