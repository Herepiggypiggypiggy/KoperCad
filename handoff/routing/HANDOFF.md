# Wire routing (WS-16): handoff

## State
- Repo herepiggypiggypiggy/kopercad, my branch `claude/project-thread-euksez` (restart from main after each merge).
- #334 (design + engine) MERGED. Follow-up PR from main on this branch: guards, merged points, reach rule. Next: duct fill, fill penalty, job order, problems.

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
