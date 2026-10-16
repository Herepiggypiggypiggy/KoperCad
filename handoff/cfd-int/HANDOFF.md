# CFD-INT handoff

Branch `claude/project-thread-irxck4` (repo kopercad). Code: `crates/novo-cfd/src/cabinet/` (mod.rs solver loop, march.rs steady march, snapshot.rs fields/devices/hot spots, warm.rs, tuning.rs, statement.rs), tests `crates/novo-cfd/tests/cabinet.rs`, examples `cabinet_bench` and `step_profile` (timing).

Measured: 5.6k cells converge in 250 s (release, 19k steps, 0.2% heat balance). 50k cells: 0.15 s/step with `pressure_tol` 1e-5, kernels use about 1.1 cores. Hence default levels 6k/20k/50k cells with wall budgets 180/420/900 s.

Known: 3D enclosure temperatures are too high (wall film suspected double counted, CFD-C2 fixing). Label stays "not validated" via `ENCLOSURE_VALIDATED=false`.

Reviewers: R1 cse_01NCdhCktKEbKPHFBvi2Tjsp, R2 cse_01SEokxUZ1MKDwyQvLm4DPys. CFD-A cse_01ES8fwPNsJAC5x14QbAkDQ9, CFD-E cse_011NTTQQUj6Z1jmUhhfWs5tz, CFD-F cse_01TEPpKazVNpMa2sasn2h3sX, CFD-C2 cse_01957w91FMssXKAxLNw5h6iv.
Disk: build only needed packages in debug; the workspace debug build fills the disk.

## Device surface vs cell count (examples/surface_study, sealed 600x800x300, one 50 W device, 2D engine says 82.3 C)
cells 462: 50.5, 896: 41.0 (both flagged "not resolved", band 25..76 / 25..57); 1890: 80.3; 3213: 82.8; 5980: 83.5 (settled within 1.5%, within 1.5% of 2D). Mean air next to the device 36.0 / 37.5 / 37.6.
Cause of the old jumps: a device under 2 x 2 x 2 cells gives a cell value, not a surface. The 2.5 mm plate is not resolved by the mesh (first z cell 38 mm); plate_share reads 0 (2D says 0.27) while the plate does conduct in-plane through the cut cell. Forcing plate planes costs 6x in the pressure solve (70+ iterations) so it waits for the faster pressure solver.
