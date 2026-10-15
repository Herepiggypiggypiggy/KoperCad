# WS-18 Revisions handoff

The WS-02 thread has owned this since 2026-10-15. The integration lead
assigned it once WS-02's queue was empty, under Tadjiro's standing
instruction of 2026-10-14 21:24 UTC to keep building toward the roadmap.
The crate is `novo-revisions`. It is already listed in
`[workspace.dependencies]`, but its directory doesn't exist yet.

## Plan
1. DESIGN PR. Adds §7 "Revisions" (what a revision is, how it ties to
   page freeze, what Changes compares), a §8 row "Checkpoints: revisions
   only", and a §10 model addition. Reviewers: the main reviewer and the
   design reviewer.
2. novo-core (WS-02): a `Revision` record (index, date, author,
   description) and a command to make one.
3. novo-storage (this thread, since WS-04 retired): a snapshot table
   keyed by revision id. Reviewer B.
4. novo-revisions:
   - the index rule and the next index;
   - issuing a revision, which writes the record and freezes every
     schematic page;
   - listing revisions;
   - diffing two projects into a per-page change set;
   - the title block fields (project revision, page revision, revision
     table).

## State
- 2026-10-15: merged: DESIGN (#142), Revision record (#157), snapshot
  table (#159), engine (#163), project info diff and taken-index skip
  (#179), core page order, DeletePage, amend_last (#167).
- Open: #185 (`Wire.hand_drawn`, `ModelError::Refused`).
- Ready locally, opens after #185: `changed_sheets(old, new)` marks
  generated sheets whose content changed, from the report fingerprints
  (#182), matched by page and position.
- Next: an issue flow in the app that pairs `IssueRevision` with
  `Session::put_snapshot` (one transaction when issues come in bulk); then
  Changes mode (WS-03 and WS-06 own the marks, WS-09 the PDF). Rules
  changes in `ChangeSet.info` may want a quieter line than name and
  description.
