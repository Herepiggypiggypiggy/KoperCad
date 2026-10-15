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
- 2026-10-15: DESIGN (#142), the Revision record and `IssueRevision`
  (#157), and the storage snapshot table (#159) are merged.
- Open: the `novo-revisions` engine PR (`IndexRule`, `diff` into a
  `ChangeSet` by page, `PageRevisions` for the title block; a page's
  re-taken freeze is not a change).
- Ready locally (branch `ws02-outputs`): generated pages sort after every
  drawn page, `ProjectRules.generated_pages`, and Remove refuses an issued
  revision.
- Next:
  - an issue flow in the app that pairs `IssueRevision` with
    `Session::put_snapshot`; wrap the snapshot writes in one transaction
    if issues ever come in bulk (Reviewer B's #159 note);
  - Changes mode (latest revision against now), with WS-03 and WS-06
    owning the marks and WS-09 owning the PDF.
