# CFD-E performance: progress

- 2026-10-16: scheduler, pool, cancel, throttle, tests (`tests/sched.rs`), `sched_bench` example.
- 2026-10-16: #576 merged. Follow-up: `Event::Progress` (unthrottled, per publish) and `DemoLadder` (multi-level stand-in) so CFD-F can build the progress bar and residual sparkline.
- 2026-10-16: #639 merged. Kernel threading PR: independent loops in flow (momentum_rhs, max_dt, divergence_max, projection assembly and correction, pressure matvec) via `flow/par.rs`, bit-identical to serial (checksum equals main's). Speedup on 3 cores only ~18% (flow_bench): Gauss-Seidel diffusion, the multigrid smoother and dot products stay serial. Left alone for CFD-A's open hooks PR: diffuse_velocity, advance_temperature.
