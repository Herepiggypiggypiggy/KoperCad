# Heat H8 handoff

Branch `claude/project-thread-775snv` (kopercad). Thread id cmsg_01TV2QyH4AYw9X2aTmgUsuhCY2DuLPR4VNhj3dZwiQJf7m. Main heat thread cmsg_01TV2QyH4AYw9X2aTmgUsuhCL7FH6aNrvjWWRxkLjoj7gk.

State: #1861 (engine) merged. The UI entry (Fans inside section) is the next PR on this branch. 3D CFD not started; needs a new branch from main after merge (no stacked PRs). Fans inside are 2D only; the 3D solver ignores `circulation_fans`.

Resume: `cargo test -p novo-thermal --test circulation` and `--lib forced`.

Code: `crates/novo-thermal/src/forced.rs` (correlations), `model.rs` (`CirculationFan`, `BlowDirection`, three new `Params` fields), `setup.rs` (`CircFace`, validation), `flow.rs` (fan faces, `circulation_flow`), `thermal.rs` (`device_h_turbulent`, `circulation_jets`, `wake_tu`, heatsink in `update_device_h`), `report.rs` (`circulation` result), app side `novo-app/src/app/heat_inside_fans.rs` (+ `HeatSettings::inside`, `apply`, panel hook in `view3d_vents.rs`, project sync in `heat_openings_store.rs`).

Traps:
- Fans live in `Params`, not `Scenario`, on purpose (115 `Scenario {` literals; H1 adds `ShellModel`).
- Honesty: continuity and the fan curve are exact properties of the solved 2D field; the jet speed field, turbulence lift and wake turbulence are planning models. The 2D flow has no momentum, so jets are added analytically on the device film, not in the air temperature field.
- Martin's jet correlation needs r/D 2.5 to 7.5: a 120 mm fan over a small device is outside it, then the free-jet speed on the flat-plate film is used.
- A fan whose jet axis is 90 mm above a device front still cools it less than one skimming it: set `z_mm` to the real mounting height.
