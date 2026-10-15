# Revision diff on the canvas: handoff

Branch `claude/project-thread-at2e2h`. First draft PR adds
`novo_revisions::change_list` (`list.rs`): flat `ChangeRow { page, id, label, kind }`
in page order, labels read from the newer state (older for removed objects).

Resume: `cargo test -p novo-revisions`. App side lives in
`crates/novo-app/src/app/changes.rs` (Changes mode, outline list); it still
shows per-page counts only, so step 1 of ROADMAP.md replaces that with rows.

Snapshot interface: `Backend::revision_snapshot(RevisionId) -> Option<Project>`
already exists; no new interface needed for the diff. Not yet agreed with the
sibling revisions thread whether a "compare two revisions" needs anything more.
