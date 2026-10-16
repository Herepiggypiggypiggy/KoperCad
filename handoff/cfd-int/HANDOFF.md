# CFD-INT handoff

Branch `claude/project-thread-irxck4` (repo kopercad). Code: `crates/novo-cfd/src/cabinet/` (mod.rs solver loop, march.rs steady march, snapshot.rs fields/devices/hot spots, warm.rs, tuning.rs, statement.rs), tests `crates/novo-cfd/tests/cabinet.rs`, examples `cabinet_bench` and `step_profile` (timing).

Measured: 5.6k cells converge in 250 s (release, 19k steps, 0.2% heat balance). 50k cells: 0.15 s/step with `pressure_tol` 1e-5, kernels use about 1.1 cores. Hence default levels 6k/20k/50k cells with wall budgets 180/420/900 s.

Known: 3D enclosure temperatures are too high (wall film suspected double counted, CFD-C2 fixing). Label stays "not validated" via `ENCLOSURE_VALIDATED=false`.

Reviewers: R1 cse_01NCdhCktKEbKPHFBvi2Tjsp, R2 cse_01SEokxUZ1MKDwyQvLm4DPys. CFD-A cse_01ES8fwPNsJAC5x14QbAkDQ9, CFD-E cse_011NTTQQUj6Z1jmUhhfWs5tz, CFD-F cse_01TEPpKazVNpMa2sasn2h3sX, CFD-C2 cse_01957w91FMssXKAxLNw5h6iv.
Disk: build only needed packages in debug; the workspace debug build fills the disk.
