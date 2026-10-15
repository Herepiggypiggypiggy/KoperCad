# UI integration for the cabinet: handoff

Updated 2026-10-15 19:00 UTC.

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-v49z7p` (restart from main after each merge).
- PR 1 (draft): `crates/novo-app/src/app/cabinet_inspector.rs`, the right-hand inspector of the Cabinet, plus `CHECKLIST.md`.
- Next: problems list strip, then faces switch (after #403).

## Resume
```
git fetch origin && git checkout claude/project-thread-v49z7p
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-app cabinet
```

## People
Roster: /mnt/project-files/coordination/threads.md. Message the reviewer with the head SHA after every push; no pushes after READY.

## Decisions
- The inspector is a new file so #403 (table face) conflicts little in `cabinet.rs`.
- Engines in open PRs are not wired until they merge; the checklist tracks them.
