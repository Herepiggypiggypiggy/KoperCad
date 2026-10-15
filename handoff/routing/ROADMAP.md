# Wire routing (WS-16): roadmap

Scope: crate `novo-routing`, pure engine that routes wires through ducts on the cabinet layout. Design: `docs/design/wire-routing.md`. Requested via the coordinator; no build, no waiting on Tadjiro.

## Done
- Design doc.
- Engine: duct network (splits at ends, crossings, T joints), anchor stubs with facing and detour, shortest path, length with slack, free-air fallback, ignored diagonal ducts. 12 tests in `crates/novo-routing/tests/routing.rs`.

- Follow-up PR: non-finite ducts ignored, route points keep only bends, reach rule (max_stub_mm, default 150) so wires don't cross the plate in free air, doc marks fill as planned. 15 tests.
- Fix PR: merge_collinear keeps a turn-back point (16 tests).

- Fill PR: `plan` (longest first, fill penalty from 80%, DuctFill, Problem::{FreeAir, GuessedWire, OverfullDuct}), `Job.diameter_mm`, `Rules.fill_limit`. 21 tests.
- Fill PR merged (#358). Follow-up PR: unsized ducts (UnsizedDuct problem, no penalty), duplicate duct ids ignored. 24 tests.
- WS-02 stores ducts as `Rail { kind: Duct, x, y, length, vertical, width, height }` (0 = unknown size); the adapter to `Duct` is mine or the app's.
- Model bridge PR: `novo_routing::model::{ducts, network}` reading WS-02 duct rails (#362). 26 tests.
- Bridge merged (#371). Connection PR: `route_connections` (pin -> plate anchor, per-enclosure plan, unrouted list). 28 tests.

## Next
1. novo-reports WireList: "Length mm" column from `route_connections` (lead accepted the small WS-13 change).
2. novo-checks: routing problems (free air, overfull and unsized duct, guessed wire) as issues.
3. Thin Cabinet canvas drawing.

## Open questions
- Default fill limit 0.5 and slack 50 mm are guesses; confirm with Tadjiro's cabinets.
- Stubs ignore devices (obstacles); only ducts block a stub.
