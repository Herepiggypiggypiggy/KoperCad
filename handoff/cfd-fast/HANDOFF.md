# CFD-FAST handoff

Branch `claude/project-thread-fvw3et` (kopercad). Draft PR: see PROGRESS.md.

State: steady path exists and works on cavities and the 6k-cell cabinet (examples `steady_probe`, `steady_cabinet` in crates/novo-cfd/examples). Not yet wired into the cabinet march; no tests yet.

Resume: `cargo run --release -p novo-cfd --example steady_cabinet -- 6000 steady 10 4` (SCHEME=upwind|vanleer env, EF, MSW, DEFER knobs). `... 6000 march` is the slow reference (about 10 minutes).

Findings
- Upwind cabinet: converges to 1e-6 in 450 iterations (5 s) against about 250 s of marching.
- Second order (VanLeer): velocity wobbles about 3 percent (plume, laminar model), heat balance within 1 to 5 percent; smooth limiter and deferred-correction damping do not help. Use window means.
- cfl 10 with energy step factor 10 is the sweet spot; cfl 30 or energy factor 100 stall.

Owners: R1 reviews (cse_01NCdhCktKEbKPHFBvi2Tjsp). CFD-E owns pressure.rs (#667), CFD-INT owns cabinet/ (#677).
