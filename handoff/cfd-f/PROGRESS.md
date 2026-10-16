# CFD-F progress

- 2026-10-16: helpers + Simulate wiring + tests (PR 1).
- 2026-10-16: PR 2: devices without loss data drawn dashed with a "no data" badge, legend totals and count, stub result labelled "not validated", own message for a lost worker. #580 merged.
- 2026-10-16: PR 3: compare (Keep as A, Δ B−A map, device table, verdict) in heat_compare.rs. Wipe and Side modes still to do.
- 2026-10-16: PR 4: Wipe A|B compare mode with draggable split, tests for heat_compare::diff (R3 note on #625). Side mode, time slider, progress bar, 3D face still to do.
- 2026-10-16: PR 5: Side compare (B on the plate, A on a copy to the right, shared viewport, auto fit). Asked the 3D view thread for an overlay-mesh entry point (set_overlays); waiting for their signature.
