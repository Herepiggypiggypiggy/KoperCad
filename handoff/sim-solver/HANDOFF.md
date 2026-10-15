# Handoff: analog solver

Branch `claude/project-thread-va2z3z`, PR from it to main. Code: `crates/novo-sim/src/analog/` (complex, matrix, circuit, solve, motor), `crates/novo-sim/src/network.rs`, app side `crates/novo-app/src/app/simulate.rs` (+ hooks in live.rs, actions.rs, app.rs, workbench.rs). Tests: `crates/novo-sim/tests/{analog,motor,network}.rs`.
Resume: `cargo test -p novo-sim`, then `cargo run -p novo-app`, Shift+L, close a contactor.
Conventions: AC values are RMS phasors; element currents run first terminal to second (a source delivering power reports negative current); node 0 is ground; 0.1 nS leakage per node; closed switch 1 µΩ.
Traps: motor torque/current are quasi-steady-state (no inrush peak, no star-delta re-close spike); the nameplate fit assumes a 40/60 stator/rotor leakage split and is an estimate; `analyse` treats conductors as ideal and a motor as its rated-load impedance; typical IE3 values fill missing nameplate data and are listed as Assumed notes.
Other lanes: 8b PLC program run not started (no thread yet); 8c faults/trip curves is thread cse_01NQC5mMueppdFcRDgtrhhwd, builds on `analog` through its own trait.
Coordinator session: session_01PgJyn3Yh6bBF3XNhYuLmpc.
