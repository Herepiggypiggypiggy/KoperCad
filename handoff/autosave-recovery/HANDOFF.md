# Autosave and crash recovery: handoff

State: #375 (storage) merged. App wiring PR from branch `claude/project-thread-ksi1p4` (restart from origin/main after each merge). After that the thread has only the follow-ups in ROADMAP.md.

Code: `crates/novo-storage/src/session.rs` (`recovery_dir`, `recovery_path()`, `set_recovery_dir`, `read_recovery`), `crates/novo-app/src/backend.rs` (`with_recovery_dir`, `recovery_dir_for`), `crates/novo-app/src/lib.rs` (`recovery_dir()`). Tests: storage `tests/recovery_dir.rs`, `tests/upgrade.rs`, app `backend::tests::recovery_files_go_to_the_recovery_folder_not_beside_the_project`.
Regenerate fixtures only for an intended format change: `KOPERCAD_REGEN_FIXTURES=1 cargo test -p novo-storage --test upgrade`.

Resume: `git fetch origin && git checkout -B <branch> origin/main && cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-storage -p novo-app`.
Traps: a recovery file beside the project from an older version must keep being offered. Untitled projects keep recovery beside their file (already in app data). Once READY, do not push to a PR.
People: integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S merges; main reviewer cse_016C9nTT8cN7voW8t1NQUG4W.
