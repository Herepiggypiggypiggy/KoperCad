# CFD-C2 solids and openings: handoff

Branch `claude/cfd-c2-solids-openings-imouuc` (repo kopercad). Reviewer R1 cse_01NCdhCktKEbKPHFBvi2Tjsp, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S, validation CFD-G cse_01DAHZ7hBowBpC9Tn8jZv5NL.

State: PR 1 (open boundaries and openings) done locally, see PROGRESS. Code: `crates/novo-cfd/src/openings/{mod,set}.rs`; hooks in `flow/solver.rs` (set_open_faces / finish_open_faces / predict_open_faces, transverse momentum flux, enthalpy flux, wall friction weight) and `flow/pressure.rs` (`set_open_conductance`). Tests: `tests/openings.rs`.

Model: face law p_face = r u + p_jump (kinematic). Vent K = 1/(Cd phi)^2 as novo-thermal; fan = line, resistance dp_max A / Q_free plus jump, evenly spread. Buoyancy reference must be the outside temperature (`FlowConfig::t_ref`). Open faces are not diffused or convected (half-cell, buoyancy only). Limits: lagged speed in the quadratic loss (smoothed); inflow tangential velocity zero; fans only on walls.

Next: PR 2 solids (see ROADMAP). Resume: `cargo test -p novo-cfd --test openings`.
