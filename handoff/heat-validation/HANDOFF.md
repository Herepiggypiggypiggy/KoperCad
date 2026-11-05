# Heat H6 validation: handoff

- Fix: `crates/novo-cfd/src/flow/solver.rs`, `FILM_REF_M` (30 mm), `film_reference`, `update_film_reference`, `q_film` (lagged source, conserves heat), `device_film_state` reads the reference air. Written up in docs/thermal-validation.md, "Mesh creep: closed by the film reference".
- Gate: `crates/novo-cfd/validation/references.txt` + `tests/validation_gate.rs`. Only passed `measured` or `licensed` references may justify `ENCLOSURE_VALIDATED`. Add a case = add a register line + a `validation_*.rs` printing `VALIDATION` rows.
- IEC: `crates/novo-thermal/tests/validation_iec.rs` (hand calc, six sizes). App follows the surface-factor method (b factors, k), not the TR 60890:2022 curves RiTherm uses.
- Traps: 30 mm is a tuning choice; 60 mm and 100 mm trials in the doc. 60000-cell run still +2.6 K. The fan what-if "filter fan in, vent out" housing moved from -1% to +14% against 2D. Vented air rise at 20000 cells not re-measured. RiTherm data is private (project files only).
- Resume: `cargo run --release -p novo-cfd --example mesh_study -- 1000 6000 20000`; `cargo test --release -p novo-cfd --test validation_mesh_creep -- --ignored`.
