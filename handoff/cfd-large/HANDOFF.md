# CFD for large cabinets: handoff

State 2026-10-22: PR open from branch `claude/project-thread-k28i3a` (see PROGRESS.md for the head).

What it does: `CabinetSolver::solve` checks the full grid (`plan_grid_report`); when it is over 1.5 times the level budget (or `LargeMode::Force`) it calls `large::reduce`, solves the reduced `Scenario` with the unchanged solver, and maps every snapshot back with `large::expand`.

Files: `large/build.rs` (blocks, lattice with clustered anchors, smearing, pitch search), `large/expand.rs` (back to devices), `large/report.rs` (LargeReport, notes, convergence verdict). UI: `novo-app/src/app/heat.rs` legend (reduced line, convergence lines).

Numbers (CV-200 with assumptions, 460 W, 4 cores): Coarse 6.5 k cells 36 s, Medium 19.6 k 44 s, Fine 47.6 k 6 min, all converged. Drive housing 89 / 95 / 86 C, margin -7.2 / -7.9 / -8.0 K; the 2D model says -15 K. Not validated against a measurement.

Resume: `cargo run --release -p novo-cfd --example large_cabinet -- FILE.novo coarse|medium|fine [solve]` (env `LARGE_DEBUG=1` prints the pitch search). Tests: `cargo test -p novo-cfd --test large` (the reduced-vs-full test takes about 100 s).

Traps
- The CV-200 file is customer-derived and stays out of the public repo; QA uses `qa/fixtures/text-stress-cabinet.novo` (59 devices, 733 k cells full).
- Reduced results depend on the grid like every other (air round the drive: 11.4 K at 3.7 k cells, 16.2 K at 5.8 k, 18 K at 10.5 k; the full run at 9.4 k cells gives 16.3 K). Reading the numbers needs the level in mind.
- QA F-001 (Cabinet toolbar scrolls sideways at 1366 on a multi-panel cabinet with the heat strip) is not from this work; the toolbar code is untouched.
- Do not push to this PR after READY; follow-ups go in a new PR from main.
