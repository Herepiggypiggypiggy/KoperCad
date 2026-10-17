# CFD-D radiation: handoff

Branch `claude/cfd-d-radiation-nlu47i`, repo kopercad, PRs from main. Reviewer R1 `cse_01NCdhCktKEbKPHFBvi2Tjsp`, lead A `cse_01FiLvRBKEcNNHbQEGhYyL1S`, coordinator `session_012zgp6RwwaD938gARNwqHHY`. PR 1 (#594) merged; PR 2 is the coupling.

Code: `crates/novo-cfd/src/rad/` (`patches.rs` patches and `RearPlate`, `trace.rs`, `viewfactor.rs`, `radiosity.rs`, `couple.rs` plan/state/load, `paths.rs`). Flow hooks: `flow/solver.rs` (`tick_radiation`, `rad_load`, sheet sweep, `wall_heat_w`) and `flow/solver/steady/assemble.rs`. Setup: `solids/setup.rs` `enable_radiation`. Result: `result.rs` `DeviceHeatPaths`. Switch: `cabinet/mod.rs` `RadMode`.
Tests: `tests/rad.rs` (engine), `tests/rad_load.rs` (plate, conservation), `tests/rad_coupling.rs` (sealed 50 W cabinet, about 50 s).

Model decisions
- Radiation replaces the film radiation of the 2D engine (`device_emissivity = 0` when enabled), so it is not counted twice.
- Device faces: temperature of the lumped device cell; sink is implicit (`s - g T`), updated every `RAD_EVERY` = 10 iterations, flux relaxed 0.7.
- Plate: thin sheet, not resolved by the mesh, so `RearPlate` makes the rear face a plate surface (columns with cover at least 0.5); device feet on the plate do not radiate.
- Walls: surface node with inner film `h_in = 1.5 U`; the part `(r-1)/r` of absorbed radiation returns to the air, the rest leaves and is added to `wall_heat_w`. Newton step per update keeps the node stable.
- Energy balance: all deposits sum to zero by construction (tested), sealed cabinet imbalance 0.08 %.

Limits: grey, diffuse, air transparent, cut-cell faces whole, emissivities and wall film ratio are planning values.

Resume: `cargo test -p novo-cfd --test rad --test rad_load --test rad_coupling`, `cargo clippy --workspace --all-targets -- -D warnings`.
