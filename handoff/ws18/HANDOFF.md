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
- 2026-10-15: DESIGN (#142), the Revision record (#157), the storage
  snapshot table (#159) and the `novo-revisions` engine (#163) are merged.
- Open: the core PR with generated page order, `generated_pages_off`,
  `DeletePage`, `Document::amend_last`, and Remove refusing an issued
  revision.
- Next, in WS-18 (the lead assigned all three):
  - diff `ProjectInfo` too (title and rule changes are not records);
  - `IndexRule::next` skips indexes already taken;
  - mark generated sheets whose content changed. Ask the Outputs thread
    for a per-sheet fingerprint.
- Later: an issue flow in the app that pairs `IssueRevision` with
  `Session::put_snapshot`, writing snapshots in one transaction when
  issues come in bulk; then Changes mode (WS-03 and WS-06 own the marks,
  WS-09 the PDF).
