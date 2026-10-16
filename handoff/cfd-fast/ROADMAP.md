# CFD-FAST roadmap

Goal: the cabinet CFD in seconds (Coarse), tens of seconds (Medium), minutes (Fine) with the same answers, error versus speed reported per change.

Done
- Steady pseudo-transient path `FlowSolver::steady_iterate` (crates/novo-cfd/src/flow/solver/steady/): implicit upwind convection, deferred correction for the configured scheme, incremental pressure correction (SIMPLEC with d = dt), own sweeps. Old `step` untouched.

Next, in order
1. Window-mean convergence monitor and averaged fields (second-order plumes do not settle).
2. Selectable first-order mode wired for the cabinet march (CFD-INT owns `cabinet/`).
3. Faster momentum assembly, pressure solve (loose tolerance, kernels; pressure.rs is CFD-E's).
4. Coarse-to-fine start across the levels.
5. Validation table: Ghia, de Vahl Davis, Churchill-Chu plate, IEC 60890 sealed box, steady versus march.

Open questions
- Second-order cabinet plumes are unsteady in the laminar model: only window means are meaningful.
