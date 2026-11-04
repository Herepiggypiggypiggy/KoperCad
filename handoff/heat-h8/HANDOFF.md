# Heat H8 handoff

Branch `claude/project-thread-775snv` (kopercad). Thread id cmsg_01TV2QyH4AYw9X2aTmgUsuhCY2DuLPR4VNhj3dZwiQJf7m. Main heat thread cmsg_01TV2QyH4AYw9X2aTmgUsuhCL7FH6aNrvjWWRxkLjoj7gk.

State: PR 1 (engine) pushed, see PROGRESS.md for the head SHA. UI (PR 2) and 3D CFD (PR 3) not started.

Resume: `cargo test -p novo-thermal --test circulation` and `--lib forced`.

Code: `crates/novo-thermal/src/forced.rs` (correlations), `model.rs` (`CirculationFan`, `BlowDirection`, three new `Params` fields), `setup.rs` (`CircFace`, validation), `flow.rs` (fan faces, `circulation_flow`), `thermal.rs` (`device_h_turbulent`, `circulation_jets`, `wake_tu`, heatsink in `update_device_h`), `report.rs` (`circulation` result).

Traps:
- Fans live in `Params`, not `Scenario`, on purpose (115 `Scenario {` literals; H1 adds `ShellModel`).
- Honesty: continuity and the fan curve are exact properties of the solved 2D field; the jet speed field, turbulence lift and wake turbulence are planning models. The 2D flow has no momentum, so jets are added analytically on the device film, not in the air temperature field.
- Martin's jet correlation needs r/D 2.5 to 7.5: a 120 mm fan over a small device is outside it, then the free-jet speed on the flat-plate film is used.
- A fan whose jet axis is 90 mm above a device front still cools it less than one skimming it: set `z_mm` to the real mounting height.
