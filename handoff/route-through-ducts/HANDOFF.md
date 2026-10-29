# Route all through ducts: handoff

- Terminal anchors: `anchor_of`, `edge_slot`, `strip_slot` in `crates/novo-routing/src/model.rs`; `Routed::estimated` lists wires with an estimated end.
- Entry choice and detour: `entries` in `crates/novo-routing/src/graph.rs` (`SIDEWAYS_MM`, `TURN_MM`).
- Cabinet drawing and notes: `crates/novo-app/src/app/cabinet_routes.rs` (free-air wires: two red rings, no line).
- Tests: `crates/novo-routing/tests/estimated_terminals.rs`, `crates/novo-app/src/app/route_through_ducts_tests.rs`, `crates/novo-tests/tests/route_through_ducts.rs`.
- QA: `qa/scenarios/1650-route-all-ducts-terminals.qa`.
- Lengths in older report tests were re-derived (756 mm for F1:2 to K1:1 in the report tests' plate).

Follow-up (slots, bodies, rails):
- Engine: `Network::with_openings`, `with_bodies`, `with_rails` in `crates/novo-routing/src/graph.rs` (`Openings`, `Body`, `RailRow`); `model::network` feeds them from the project (`duct_openings`, `bodies_and_rows`).
- Stub path: `stub_path` and `stub_path_to` (candidates: two L shapes, two wraps round the own device; a slotted duct is entered across it at the slot).
- `Route::rails` and `Route::blocked`; `Problem::StubThroughDevice`; `ProblemKind::WireThroughDevice`.
- UI: Cabinet > Rails and ducts > "Slots" column (`cabinet_rails.rs`), notes in `cabinet_routes.rs`.
- Tests: `crates/novo-routing/tests/bodies_slots_rails.rs`.
