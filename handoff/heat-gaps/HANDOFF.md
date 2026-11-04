# Heat H4: handoff

Code: crates/novo-thermal/src/gaps.rs, tests/gaps.rs; app: heat_settings.rs (`VentSpec.filter`, `filter_load`, `HeatSettings.seal`), heat_openings_store.rs (text is the bare list, or `{vents, seal}` once a seal is set), view3d_vents.rs (editor rows, `seal_row`).
Model status: all class tables are planning values, labelled "model"; IEC 60529 gives no air rate. Leaks are power law (exponent 2/3) turned into effective leakage area at 4 Pa; leak vents are appended after the user's vents so indices of user openings do not move.
Traps: filter on a grille is exact at 4 Pa only; seal changes are not in the panel's undo stack (vents only); FilterFan kind ignores the mat (datasheet has it).

Real enclosure feed (heat coordinator 2026-11-04): the chain thread's catalogue record should fill `gaps::Seal { ip, glands }` (IP rating, gasket, knock-outs and gland plates as the gland count), and the filter mats and vents as `VentSpec` rows. Today the seal is set by the person in the Vents and fans panel and is off by default; when the record has no IP rating the panel must say "assumed" next to the class (not built yet). Provenance per field (catalogue / measured / assumed) belongs in the record, not in `gaps.rs`.
