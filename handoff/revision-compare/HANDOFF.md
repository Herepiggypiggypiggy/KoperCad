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

## App PR (claude/revision-compare-app)
Compare window (`crates/novo-app/src/app/compare.rs`), command
`revision.compare` ("Compare revisions", verb `compare`), and a "Compare…"
button on the Changes card.

Revision view (PR claude/revision-view): "Show drawing" in the Compare
window draws one side (From or To, any page) read only with
`PageScene` + `CanvasView`, changes outlined with canvas marks (added on To,
removed on From, changed on both). How to reach it: command palette
"Compare revisions" (or Changes card "Compare…") → tick "Show drawing".

Status (2026-10-15 evening): merged #400, #435 (engine), #445 (CLI), #450
(Python), #468 (Compare window). Open: #487 (revision view) and #493
(sheet kind `match` in novo-py, reviewer nit from #450), both with reviewer B.

Next, for a fresh session: (1) note field. WS-02's #490 adds
`note: String` to `Command::IssueRevision` (stored in
`Revision.properties["note"]`, read with `Revision::note()`); after it
merges, add a note box to the Issue revision window, show `note()` in the
Compare window pickers and in `revisions list`, and pass it through
`issue_revision_as`. (2) Workspaces thread moves the Issue/Compare menu
items under File > Revisions; keep the `revision.*` command ids. (3) Reviewer
leftovers: old-format snapshot test once a migration exists, fold batch moves
("12 objects moved together"), `Side::Now` clones the project (borrow if slow).
The revision table in the frame is #343 (Frame thread, not this one).

## How to reach it (user-visible entry points)

- Terminal: `kopercad-cli revisions list <file>` and
  `kopercad-cli revisions diff <file> <from> <to> [--json]` (#445).
- Python: `Project.revisions()`, `Project.compare("A", "now")` (#450).
- App (#468, merged): command `revision.compare` (Ctrl+K "compare
  revisions", or `compare A B`) opens the Compare window; the Changes card
  has a "Compare…" button. The clickable change list and PDF report are the
  Revision diff thread's.
