# CFD-F progress

- 2026-10-16: helpers + Simulate wiring + tests (PR 1).
- 2026-10-16: PR 2: devices without loss data drawn dashed with a "no data" badge, legend totals and count, stub result labelled "not validated", own message for a lost worker. #580 merged.
- 2026-10-16: PR 3: compare (Keep as A, Δ B−A map, device table, verdict) in heat_compare.rs. Wipe and Side modes still to do.
- 2026-10-16: PR 4: Wipe A|B compare mode with draggable split, tests for heat_compare::diff (R3 note on #625). Side mode, time slider, progress bar, 3D face still to do.
- 2026-10-16: PR 5: Side compare (B on the plate, A on a copy to the right, shared viewport, auto fit). Asked the 3D view thread for an overlay-mesh entry point (set_overlays); waiting for their signature.
- 2026-10-16: PR 6: Simulate runs through the novo-cfd scheduler (Coarse, Medium, Fine), progress bar and residual sparkline in the Heat toolbar (heat_progress.rs). Default solver is the demo ladder (synthetic iterations, says so in its notes) until the flow core lands; swap point heat_run::default_solver. #633 and #641 merged. Renderer thread builds View3d::set_overlays (Mesh/Lines); 3D face waits for it.
- 2026-10-16: PR 7: 3D face. Heat strip above the 3D view (Slice X/Y/Z with position, Shell at a temperature, Airflow) from the last Simulate result, as View3d overlays (heat_3d.rs, built on #647). Time slider and animated tracers still to do.
- 2026-10-16: PR 8: animated tracers on the 3D airflow lines (bright dashes moving downwind, faster lines cycle faster, still when reduce motion is on) and an asymmetric-layout test for the plate-frame mapping (R3 note on #664). Time slider waits for transient frames from CFD-A. #652 and #664 merged.
- 2026-10-16: handoff brought up to date after #652, #664, #666 merged; CabinetSolver is the default solver.
- 2026-10-17: PR 9: time slider. Heat-up button (length in minutes) runs a TransientSolver (2D stub for now, swap point heat_time::default_solver); Play, slider, load share and an air-temperature curve with a cursor under the Simulate controls (Cabinet strip and 3D Heat panel). Turbulence switch was already done by the 3D thread (#714).
