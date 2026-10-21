# Revision lock and history: handoff

Branch: claude/project-thread-73pred (kopercad). PR: see PROGRESS.md for the number and head SHA.

State: lock, restore and UI are written; see ROADMAP.md for what is next.

Where the code is:
- Lock: `crates/novo-core/src/tx.rs` (`lock_page`, `unlock_page`, `refuse_locked`, `allow_locked_pages`), `project.rs` (`transact` takes the pre-edit set of locked pages), `command.rs` (`LockPage`, `UnlockPage`), `entity.rs` (`Page.locked`).
- Issue with lock: `crates/novo-revisions/src/issue.rs` (`issue_revision_locked`).
- Restore: `crates/novo-revisions/src/restore.rs`. Records are applied in a retry loop because the model checks references op by op.
- App: `crates/novo-app/src/app/revisions.rs` (dialog box, lock and restore commands), `compare.rs` (Restore buttons).
- Tests: `crates/novo-revisions/tests/restore.rs`.

Rules the code follows:
- A page lock refuses edits to the page record, its symbols, junctions, wires, texts, block instances, and updates to devices, terminals and cables that are drawn on it. Changing only `locked` or `frozen` on a page is allowed. Project info edits are not guarded.
- Restore never touches revisions, ignores locks (it is the way back to what was issued) and is one undo step.
- `diff` ignores `locked` on pages like `frozen`.

Resume: `cargo test -p novo-revisions -p novo-core`; then fmt and clippy as in CLAUDE.md. Use one CARGO_TARGET_DIR.
Core API change (Page.locked, two commands, Tx::lock_page): described in the PR for WS-02.
