# Cooling selection handoff

- Engine: `crates/novo-thermal/src/cooling.rs`. Need = loss - k*A*(target - ambient) (IEC 60890 surface, `novo_layout::sizing`). Fan flow = FAN_FACTOR*need/delta, derated 0.9 (fan) or 0.7 (filter fan); units need 1.1x. Fans are out when delta < 3 K or the enclosure must stay closed.
- Products: parts with `airflow_m3h` / `cooling_capacity_w` (optional `cooling_kind`), the person's `cooling.csv` (`name;kind;airflow_m3h;cooling_w`, under %APPDATA%\KoperCAD), then generic planning sizes labelled "planning size".
- App: `crates/novo-app/src/app/heat_cooling.rs`; shown in `view3d_heat.rs` (3D Heat panel) and the legend of `heat.rs` (Cabinet Heat view). Result carries the enclosure (`Solution.enclosure`). Report lines go through `HeatFact.cooling` into the standards report.
- Wire heat is not merged: `Basis.loss_w` takes it once the caller adds it.
- QA: `qa/scenarios/44-cooling-selection.qa`; the example only loses 14 W so it shows "No cooling needed". Proposals are covered by the hand-calculated tests.
