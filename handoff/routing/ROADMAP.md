# Wire routing (WS-16): roadmap

Scope: crate `novo-routing`, pure engine that routes wires through ducts on the cabinet layout. Design: `docs/design/wire-routing.md`. Requested via the coordinator; no build, no waiting on Tadjiro.

## Done
- Design doc.
- Engine: duct network (splits at ends, crossings, T joints), anchor stubs with facing and detour, shortest path, length with slack, free-air fallback, ignored diagonal ducts. 12 tests in `crates/novo-routing/tests/routing.rs`.

- Follow-up PR: non-finite ducts ignored, route points keep only bends, reach rule (max_stub_mm, default 150) so wires don't cross the plate in free air, doc marks fill as planned. 15 tests.
- Fix PR: merge_collinear keeps a turn-back point (16 tests).

## Next
1. Duct fill (cross-section area, fill limit), fill penalty in the path cost, job order longest first, problems (overfull duct, free air).
2. Ask WS-02 for duct and pin storage; commands.
3. Wire list lengths (novo-reports), checks (novo-checks).
4. Thin Cabinet canvas drawing.

## Open questions
- Default fill limit 0.5 and slack 50 mm are guesses; confirm with Tadjiro's cabinets.
- Stubs ignore devices (obstacles); only ducts block a stub.
