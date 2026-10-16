# CFD-C2 solids and openings: handoff

Branch `claude/cfd-c2-solids-openings-imouuc` (repo kopercad). R1 cse_01NCdhCktKEbKPHFBvi2Tjsp, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S, lead B (merger) cse_011NKUp2tgBn8uVWK6WHsc7k, CFD-G cse_01DAHZ7hBowBpC9Tn8jZv5NL.

State: PR 1 (openings) merged as #603. PR 2 (solids, conjugate heat transfer, scenario driver) open, see PROGRESS for the head. Lead A: the scenario-to-FlowSolver driver is mine (solids/setup.rs).

Code: `crates/novo-cfd/src/solids/{mod,props,setup}.rs`; flow hooks in `flow/solver.rs` (`FlowConfig::medium`, `afrac`, `advance_temperature`) and `flow/pressure.rs` (`with_openness`, active mask). Tests: `tests/solids.rs`, `tests/openings.rs`.

Model: face open = min of the two air shares, closed below 0.1; cell k and heat capacity share-weighted; face k harmonic; devices heated uniformly over their volume; device k = 2 W/mK and capacity 500 J/K per dm2 are planning values; walls: IEC 60890 k b A_out / A_in as film (adiabatic bottom when on the floor); `solid_speedup` divides solid heat capacity for steady marches only.

Limits: laminar (no turbulence coupling yet), no radiation (CFD-D), ducts are solid PVC blocks until PR 3 (porous), partially open faces use min share (cut-cell lite), slow: see PROGRESS note for CFD-E.

Resume: `cargo test -p novo-cfd --test solids -- --nocapture`; probe: `cargo run -p novo-cfd --example solids_probe 50 1500 6000`.
