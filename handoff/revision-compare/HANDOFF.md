# Revision compare handoff

Design: docs/design/revision-compare.md (#400, READY). Existing revision
work is WS-18 (handoff/ws18). Roster: coordination/threads.md.

Agreement with the Revision diff thread (#401): one row type, `ChangeRow`
(list.rs), which gained `fields: Vec<FieldChange>`. This thread owns the
compare engine, list.rs, CLI and Python; that thread owns the app list,
PDF change report, overlay and the From/To pickers. Ping them when the
engine merges.

State: engine PR (branch claude/revision-compare-engine, based on #401):
`compare`, `Side`, `Comparison`, `CompareError`, `FieldChange`,
`field_changes`, `Group`/`group_of`, unnumbered wire labels by their ends.
Next: CLI `revisions list|diff` (novo-cli) and the Python call (novo-py),
then the reviewer notes on #400: old-format snapshot test once a migration
exists (storage returns an error today, compare reports `Unreadable`),
name the cache counter when the app caches, batch-move folding.

Resume: `cargo test -p novo-revisions`; fmt and clippy as CLAUDE.md.
Rules: PRs stay drafts, no builds, no push after READY, message the
reviewer with the head SHA after each push.
