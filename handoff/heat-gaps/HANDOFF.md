# Heat H4: handoff

Code: crates/novo-thermal/src/gaps.rs, tests/gaps.rs; app: heat_settings.rs (`VentSpec.filter`, `filter_load`, `HeatSettings.seal`), heat_openings_store.rs (text is the bare list, or `{vents, seal}` once a seal is set), view3d_vents.rs (editor rows, `seal_row`).
Model status: all class tables are planning values, labelled "model"; IEC 60529 gives no air rate. Leaks are power law (exponent 2/3) turned into effective leakage area at 4 Pa; leak vents are appended after the user's vents so indices of user openings do not move.
Traps: filter on a grille is exact at 4 Pa only; seal changes are not in the panel's undo stack (vents only); FilterFan kind ignores the mat (datasheet has it).
