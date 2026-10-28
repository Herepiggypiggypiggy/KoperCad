# Heat hang and empty heat map: roadmap

Scope: findings 6, 7 and 33 of the cooling tower laptop test, plus three 3D heat points from Tadjiro (slice cut-outs, cloud too subtle, simulation status in the 3D view).

Done (branch `claude/heat-hang-empty-map`):
- Meshing hang: `enforce_ratio` (novo-cfd mesh/axis.rs) crept without end on alternating cell sizes; bounded cascade plus optimal-split fallback.
- Cancel: checks between setup stages and in the pitch search (`reduce_with`), named stages (`Control::set_phase`), "Stopping…" state and let-go after 3 s.
- Vent and fan editing is no longer locked during a run.
- No loss data on any device: no field, no numbers, a line saying why; "Cannot simulate" line cleared once the enclosure is sized.
- 3D slice: device cells drawn darker at the device temperature, not left out.
- Cloud: own scale (ambient to the hot end of the air) with its own key, Strength and Contrast sliders.
- `HeatState::sim_status()` (heat_status.rs) for the 3D status chip.

Next:
- 3D rework thread builds the always-visible chip on `sim_status()`.
- Heat lessons re-reviewed for numbers and visuals (heat lesson thread).
- Open: the Coarse level of a cabinet whose bodies alone exceed the cell budget (88 000 cells against 6 000) still runs a grid over budget; it finishes (about 1 minute) but a better planner would cut devices into fewer planes.
