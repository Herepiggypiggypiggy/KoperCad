# CFD-B turbulence: handoff

Branch `claude/cfd-b-turbulence-9reum0` (repo kopercad). Engine lives in `crates/novo-cfd/src/turb/` (mesh.rs wall distance and wall faces, wall.rs Spalding/Kader, transport.rs implicit upwind scalar solver, mod.rs SST and k-epsilon). Tests: `tests/turb_basics.rs`, `tests/turb_channel.rs` (shared driver `tests/common_channel.rs`).

API: `TurbState::new(grid, solid, TurbConfig, ref_speed)`, `advance(face_velocities, temperature, dt, sweeps)`, then read `nu_t`, `viscosity_ratio()`, `alpha_t()`, `walls()` (WallCell: u_tau, y_plus, nu_wall, alpha_wall). Velocity uses the MAC layout of the flow core (`n[c]+1` samples along axis c).

Validated (Re_tau 590 channel, errors in the tests, run with `--nocapture`):
- SST, resolved wall (y+ 0.5): log-region error 4.7 % vs Reichardt, centreline U+ 20.69 vs DNS 21.3 (-2.9 %).
- SST, wall function (first cell centre y+ 21): 2.7 %, U+cl 20.61.
- k-epsilon, wall function: 7.1 %, U+cl 20.42; SST vs k-epsilon centreline difference 0.9 %.

Traps
- Start the wall layer from the log-law guess (`initialise_wall_layer`, done in `new`); a uniform start falls onto a laminar-like branch on coarse grids.
- Wall-cell strain is overridden by u_tau^2 / (nu + kappa u_tau d); the no-slip difference over half a cell overestimates it five-fold.
- Wall distance is exact for domain walls, a fast-sweeping eikonal for solid cells (slightly low at wall-wall junctions).

Next: hooks in the flow core, see ROADMAP.md. Reviewer R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`.
