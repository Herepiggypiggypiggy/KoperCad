# Wire routing (WS-16): handoff

## State
- Repo herepiggypiggypiggy/kopercad, my branch `claude/project-thread-euksez` (restart from main after each merge).
- Checks PR on this branch (`novo_checks::check_routing`, takes the wire list's connections); earlier PRs merged. Next: the app calls check_routing with the wire list connections once the wire list thread exposes them; then thin cabinet canvas drawing.

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
