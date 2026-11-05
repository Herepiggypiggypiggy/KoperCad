# Heat on the chosen enclosure (CHAIN.md, Heat/CFD row)

## 2026-11-04
- `novo-thermal` bridge: when the cabinet Structure carries an `encl.` record (`novo_layout::spec`, E1 #1891), outside size, wall and plate setback come from it; the typed `wall_mm` option is ignored then. A missing size or wall fails with `BridgeError::SpecIncomplete` ("The enclosure has no wall thickness."), never a default. No record: behaviour unchanged.
- `BridgeReport.enclosure_source/article` carry the weakest source; heat messages show "Enclosure <article>: catalogue" and the estimate notice.
- Tests: `crates/novo-thermal/tests/bridge.rs` module `spec` (a bigger spec runs cooler). QA 9730-9733 (small and large enclosure, 1366x768 and 1920x1080) with synthetic fixtures (`qa/fixtures/make_heat_spec_fixtures.py`).
- Not yet: openings and fans at real positions, shell layers (#1865), wall material/finish from the spec; they wait for the STEP body reader (E2/E4) and #1865.
