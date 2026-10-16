# CFD-B turbulence: handoff

Branch `claude/cfd-b-turbulence-9reum0` (repo kopercad), always restarted from main after a merge. Engine `crates/novo-cfd/src/turb/`: mesh.rs (wall distance, wall faces), wall.rs (Spalding, Kader), transport.rs (implicit upwind scalar solver), mod.rs (SST, k-epsilon), coupling.rs (`TurbulentFlow`: sets `flow.nu_t`, `flow.wall_nu`, `flow.wall_alpha` each step, then advances the turbulence). Tests: turb_basics, turb_channel, turb_coupling (fast); turb_plate (slow, `--ignored`).

Validated (numbers printed by the tests):
- Channel Re_tau 590: SST resolved 4.7 % log-layer error, centreline U+ 20.69 vs DNS 21.3; SST wall function 2.7 %; k-epsilon 7.1 %; the two agree within 1 %.
- Heated wall vs Churchill-Chu (Nu of wall-to-core temperature difference): SST resolved +4.3 % (Ra 1.0e9, 40x80, 16 min) and +9.8 % (Ra 3.4e9, 28x56); k-epsilon on the resolved grid +34.5 % (not made for resolved buoyant layers); coarse wall cells 16x48: SST and k-epsilon agree (85.6 vs 85.0) but are 52 % low. State this in the accuracy text: wall functions are not valid for natural-convection boundary layers thinner than the first cell.
- Hot/cold wall flux mismatch in the plate runs is 5 %.

Traps
- Start the wall layer from the log-law guess (`new` does it); a uniform start falls on a laminar-like branch.
- Wall-cell strain is overridden by u_tau^2/(nu + kappa u_tau d).
- `StepInfo` of the flow core is not exported, so `TurbulentFlow::step` returns `(max_accel, max_dtdt)`.
- The wall functions use the absolute tangential speed (moving walls not subtracted yet).
- Plate test with the cavity uses the core temperature at mid height; it is a cavity, not an infinite plate.

Contacts: R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, CFD-A `session_01ES8fwPNsJAC5x14QbAkDQ9`.
