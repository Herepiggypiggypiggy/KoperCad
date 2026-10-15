# Autosave and crash recovery: handoff

Branch `claude/project-thread-ksi1p4` (restart from origin/main after each merge). PR 1: novo-storage recovery folder + upgrade tests + design note. App wiring is not done; backend.rs belongs to the App shell thread.

Code: `crates/novo-storage/src/session.rs` (`recovery_dir`, `recovery_path()`, `read_recovery`), tests `tests/recovery_dir.rs`, `tests/upgrade.rs`, fixtures `tests/fixtures/`. Regenerate fixtures only for an intended format change: `KOPERCAD_REGEN_FIXTURES=1 cargo test -p novo-storage --test upgrade`.

Resume: `git fetch origin && git checkout -B <branch> origin/main && cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-storage`.
Trap: a recovery file beside the project from an older version must keep being offered.
