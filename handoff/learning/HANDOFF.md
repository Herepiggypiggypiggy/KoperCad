# Learning modules: handoff

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-7lpw0d` (restart from main after each merge).
- Merged: design (#456), tracks and glossary (#474). Open: engine PR (crate `novo-learn`: markup, lesson format, checks, runner, catalog, lint, progress; 46 unit tests plus `tests/repo_lessons.rs`) with `assets/lessons/README.md`.
- Next: app shell (card, pointer overlay, Learn window, entries, event funnel and signals, anchors), after #428 merges (it owns `onboarding.rs`; do not edit it before then). The crate is not yet in `[workspace.dependencies]`; the lead registers it when `novo-app` needs it.
- Content threads write lessons as drafts against `assets/lessons/README.md`.

## Resume
```
git fetch origin && git checkout claude/project-thread-7lpw0d
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-learn   # once the crate exists
```
Read first: the design doc, `docs/ONBOARDING-DESIGN.md` (walk-through, PR #428), `docs/design/discoverability.md` (PR #434), `crates/novo-core/src/json.rs` (project JSON form the sample projects use).

## People
Roster: /mnt/project-files/coordination/threads.md. Coordinate with Sample project and first-run walk-through (cse_019sSSXcKhY8k5uYwY3cgSyf), Onboarding and discoverability (cse_01CpKBFnbxA5RuJyh1pGkTdZ), Workspaces (cse_01WAQSEGkMbyreCPG18tsSPc), Design system thread. Message the reviewer with the head SHA after every push; no pushes after READY.

## Decisions
- Lessons are TOML, sample projects are the existing diffable project JSON; both embedded by `novo-app/build.rs`, no Rust edit to add a lesson.
- Pure engine in a new crate `novo-learn` (depends on `novo-core` only). Crate registration is the coordinator's job (CLAUDE.md).
- Checks are events (commands, tools, panels) plus document state; lessons are replayed headless in CI with each step's `solve`.
- Progress in `%APPDATA%\KoperCAD\learn.json`, not the settings blob.
