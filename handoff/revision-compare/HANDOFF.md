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

Status (2026-10-17): merged #400, #435 (engine), #445 (CLI), #450 (Python),
#468 (Compare window), #487 (revision view), #493. Revision note (PR
claude/revision-note, on top of WS-02's #490): the Issue revision window has
a "Note (optional)" box (`issue_revision_noted`, stored as
`Revision.properties["note"]`), the Compare window shows the note of the
From/To revisions, `kopercad-cli revisions list` prints it and the JSON
carries `note`.

Left: (1) Workspaces thread moves Issue/Compare under File > Revisions;
keep the `revision.*` ids. (2) Reviewer leftovers: legend in words for the
outlines and pane height in the drawing view, an Added/Removed case in the
drawing test, check the first-frame zoom-to-fit in a real window, old-format
snapshot test once a migration exists, fold batch moves, `Side::Now` clones
the project. The revision table in the frame is #343 (Frame thread).

## How to reach it (user-visible entry points)

- Terminal: `kopercad-cli revisions list <file>` and
  `kopercad-cli revisions diff <file> <from> <to> [--json]` (#445).
- Python: `Project.revisions()`, `Project.compare("A", "now")` (#450).
- App (#468, merged): command `revision.compare` (Ctrl+K "compare
  revisions", or `compare A B`) opens the Compare window; the Changes card
  has a "Compare…" button. The clickable change list and PDF report are the
  Revision diff thread's.
