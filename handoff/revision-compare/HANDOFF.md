# Revision compare handoff

Branch: claude/project-thread-udo78r (kopercad). Owner: this thread, started
2026-10-15 from the coordinator's brief. Existing revision work is WS-18
(handoff/ws18): issue, snapshots, `diff`, Changes mode, title block fields.

State: design note written and PR opened as draft. No code yet.

Resume: read docs/design/revision-compare.md, then build step 2 in
crates/novo-revisions (new `compare.rs`; reuse `diff`, `changed_sheets`,
`Session::snapshot`). Do not change novo-core: the note is a `properties`
entry. Run `cargo fmt --all`, clippy with -D warnings, `cargo test --workspace`.

Rules: PRs stay drafts, no builds, no push after READY, message the reviewer
with the head SHA after each push (roster: handoff/threads.md).
Related: #343 revision table (draft, frame owner).
