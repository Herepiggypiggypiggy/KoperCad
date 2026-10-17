# CFD-FAST roadmap

Goal: the cabinet CFD in seconds (Coarse), tens of seconds (Medium), minutes (Fine) with the same answers, error versus speed reported per change.

Done
- Steady pseudo-transient path `FlowSolver::steady_iterate` (crates/novo-cfd/src/flow/solver/steady/): implicit upwind convection, deferred correction for the configured scheme, incremental pressure correction (SIMPLEC with d = dt), own sweeps. Old `step` untouched.

- Window-mean convergence, averaged fields, grid sequencing (`initialise_from`), vent/fan open faces, strided pressure stencil.

Next, in order
1. Selectable first-order mode wired for the cabinet march (CFD-INT owns `cabinet/`).
2. Faster momentum assembly, pressure solve (loose tolerance, kernels; pressure.rs is CFD-E's).
3. Coarse-to-fine start across the levels.
4. Remaining validation: Churchill-Chu plate, IEC 60890 sealed box (cavities and cabinet vs march are in PR #688).

Open questions
- Second-order cabinet plumes are unsteady in the laminar model: only window means are meaningful.

Done 2026-10-16: 3D heat result tools (probe, key, slice position, arrows, hot spots). Next if asked: slice plane dragged in the view, probe on streamlines, key with the arrow scale.
