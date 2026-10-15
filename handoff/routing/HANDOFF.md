# Wire routing (WS-16): handoff

## State
- Repo herepiggypiggypiggy/kopercad, my branch `claude/project-thread-euksez` (restart from main after each merge).
- Connection PR on this branch (route_connections); earlier PRs merged. Next: WireList Length column, then checks.

## Resume
```
git fetch origin && git checkout claude/project-thread-euksez
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-routing
```
Read: CLAUDE.md, docs/design/cabinet-layout.md, crates/novo-layout (Rect, Placement, Rail).

## People
Roster with session ids: /mnt/project-files/coordination/threads.md. Message the main reviewer with the head SHA after every push; no pushes after READY.

## Decisions
- Routes are derived, not stored; only pins are stored.
- Free-air fallback is flagged, never length zero.

## UI entry points needed (engine is not done without them)
Sent to "UI integration: cabinet, 3D, enclosure" (2026-10-15):
1. Cabinet: "Route wires" per enclosure via `route_connections(project, &connections, &Rules::default())`.
2. Cabinet drawing: route polylines over the plate, ducts tinted by fill ratio, selecting a duct lists its wires.
3. Wire list: Length column from `Route.length_mm` (wire list thread); unrouted shows no length.
4. Issues: free air and overfull duct will come from `check()` once #413 merges; unsized duct and guessed wire infos follow after #413.
5. Add and size a duct on the Cabinet (rail of kind Duct, `set_rail_size`).
Open: whether a `route_project(project)` that builds the connections itself is wanted.
