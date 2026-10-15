# Handoff: analog solver

Branch `claude/project-thread-va2z3z`. Engine in `crates/novo-sim/src/analog/` (complex, matrix, circuit, solve, motor); tests `crates/novo-sim/tests/{analog,motor}.rs`.
Resume: `cargo test -p novo-sim`. Conventions: AC values are RMS phasors; element currents run first terminal to second (a source delivering power reports negative current); node 0 is ground; 0.1 nS gmin per node.
Traps: motor torque/current are quasi-steady-state (no inrush peak, no re-close spike); the nameplate fit uses a 40/60 leakage split and is an estimate.
Coordinator session: session_01PgJyn3Yh6bBF3XNhYuLmpc.
