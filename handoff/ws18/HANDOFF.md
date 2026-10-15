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
- 2026-10-15: DESIGN merged (#142).
- Revision record and `IssueRevision` PR open from `claude/data-model-amzaxu`.
- Ready locally, opened one at a time after it:
  - the `novo-revisions` engine: `IndexRule`, `diff` into a `ChangeSet`
    by page;
  - the novo-storage snapshot table: schema v2, zlib project JSON per
    revision id, `Session::put_snapshot`, save as copies snapshots, and
    orphans are dropped on open when no recovery is offered.
- Next: title block fields (project revision, page revision from change
  sets, revision table), then an issue flow that pairs the command with
  `put_snapshot`.
