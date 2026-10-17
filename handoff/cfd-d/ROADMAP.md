# CFD-D radiation: roadmap

Scope: grey surface-to-surface radiation in `crates/novo-cfd/src/rad/` and its coupling to the solver.

Done
- PR 1 (#594): patches, ray-sampled view factors, radiosity, `HeatPaths`.
- PR 2 (coupling): plate as a radiating surface (`RearPlate`, `PatchKind::Plate`), `RadPlan`/`RadState`/`RadLoad`, hooks in `flow/solver.rs` (implicit `s - g T` per cell and plate column, wall outflow in `wall_heat_w`), `FlowSetup::enable_radiation`, `DeviceReport::radiation_w`, `CfdSnapshot::heat_paths` and `radiation_on`, `CabinetSolver::with_radiation(RadMode)` (Auto = Fine only), accuracy statement text.

Next
1. Simulate dialog toggle and the heat split in the device card (CFD-F reads `RadMode`, `heat_paths`).
2. Validation entries for CFD-G: sealed 50 W cabinet with and without radiation (surface 85 vs 71 C at 1500 cells), enclosure against IEC 60890 with radiation.
3. Emissivity from part data (`novo-parts` key) instead of planning values; wall inner-film ratio (`WALL_INNER_RATIO`) from measurements.
4. Radiation on Coarse/Medium if the cost allows (view factor build is a one-off, the update every 10 iterations is cheap).
5. Partial plate cover (columns below 0.5 are bare wall today); cut-cell face areas.

Open questions
- The 2D engine's film radiation (view factor 0.5 against the air temperature) and the explicit exchange differ by about 15 K on a 50 W device; which one the validation should favour needs a measurement.
