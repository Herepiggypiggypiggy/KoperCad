# Wire routing (WS-16): roadmap

Scope: crate `novo-routing`, pure engine that routes wires through ducts on the cabinet layout. Design: `docs/design/wire-routing.md`. Requested via the coordinator; no build, no waiting on Tadjiro.

## Done
- Design doc.

## Next
1. Engine: duct graph, anchor stubs, shortest path, length with slack, tests.
2. Duct fill, fill penalty, problems.
3. Ask WS-02 for duct and pin storage; commands.
4. Wire list lengths (novo-reports), checks (novo-checks).
5. Thin Cabinet canvas drawing.

## Open questions
- Default fill limit 0.5 and slack 50 mm are guesses; confirm with Tadjiro's cabinets.
