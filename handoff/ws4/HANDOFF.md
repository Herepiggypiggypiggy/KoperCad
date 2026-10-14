# WS-04 storage: handoff

State: the workstream is complete and the thread is retired (2026-10-14). The only open item is PR #33.

## Branch / PR
- Branch: `claude/project-thread-l2hop0`, restarted from main at 11363c1.
- PR #33 https://github.com/Herepiggypiggypiggy/NovoElectric/pull/33, head 7b573d57cac2b1394026268facba4e2a92f6c83b.
  - ubuntu and installer are green. windows was running at 16:58 UTC.
  - Sent to the main reviewer; no verdict yet.
  - The integration lead merges it after READY and green CI.
  - Do not push to it after READY. Further work goes in a new PR from main.

## Code map (crates/novo-storage)
- `file.rs` ProjectFile: SQLite file settings
  - application_id 0x4E564F45; locking_mode EXCLUSIVE; journal DELETE; synchronous FULL; quick_check on open.
  - create writes `<path>.saving`, then `replace()`. On Unix the lock is held through the rename; on Windows it is released first.
- `session.rs` Session: autosave to `<path>.autosave`, keyed by base_seq + base_file (the main file's file_id).
  - Recovery is offered only on a match. An unreadable recovery file is renamed to `.autosave.broken`.
  - `recovery_pending` blocks autosave and the delete on close.
- `schema.rs`: MIGRATIONS list. Before migrating, a `VACUUM INTO <file>.v<N>.bak` backup is taken; all steps run in one transaction.
- `tests/storage.rs`: integration tests.

## Resume
```
git fetch origin && git checkout -B <new-branch> origin/main
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-storage
```

## People (coordination/threads.md)
- Integration lead (merges): session_011PfTJ1QaP1JQJPwXcFnxBE
- Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W
- WS-01 app shell: cse_01AYuFPxz6eQReQdtzYKLMmW. Owns `novo-app/src/backend.rs`, which uses Session.
- WS-02 data model: cse_01B1jpjzeDiXvWoBCsv1BWt9. The only thread that changes the novo-core API.

## Traps / decisions
- No admin rights: write only next to the project file or under the user profile.
- SMB/NFS: SQLite locks are unreliable there; this is documented and not solved.
- Integration with the app (PR #34): WS-01 fixed two leaks on untitled projects.
  - close_session no longer deletes the untitled file while a recovery is pending.
  - Discard calls discard_recovery() before close().
  - Both depend on #33.
- A record format change needs a migration, never a silent reinterpretation.
