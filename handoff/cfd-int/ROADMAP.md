# CFD-INT roadmap

Scope: the real solver behind the Heat layer (Simulate).

Done in PR 1: `novo_cfd::cabinet::CabinetSolver` (mesh, solids, ducts, openings, flow march, warm start from 2D, progress, snapshot), app wiring, demo ladder behind feature `demo`, `AccuracyStatement::validated`.

Next, in order:
1. Keep "not validated" until CFD-C2 fixes the sealed-box wall film (14.5 K vs 6.3 K IEC) and the ignored test in #668 passes; then add `tests/validation_cabinet.rs`, flip `ENCLOSURE_VALIDATED`, state per-level accuracy from the measured mesh study.
2. Transient frames in the result contract: CFD-A (cse_01ES8fwPNsJAC5x14QbAkDQ9) was asked to take it on top of this PR.
3. Raise Tuning cell counts (50k/500k/2M design) when kernel threading (CFD-E) and a steady-state method land.
4. Turbulence (`turb`) and radiation (`rad`) coupling at Fine (needs a radiative source hook in FlowSolver).
5. Grid sequencing Coarse -> Medium.
