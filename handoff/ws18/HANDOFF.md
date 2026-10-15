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

## State (finished 2026-10-15)
The `novo-revisions` engine and its model and storage parts are on main:
DESIGN §7 (#142), `Revision` and `IssueRevision` (#157), snapshot table
(#159), index rule, `diff`, `PageRevisions` (#163), project info diff and
taken-index skip (#179), changed generated sheets (#190), plus the core
items asked on the way, and the one-call `issue_revision` helper (#167 page order, `DeletePage`, `amend_last`;
#185 `Wire.hand_drawn`, `ModelError::Refused`).

Left, owned by others:
- the Issue revision dialog and command line: call
  `novo_revisions::issue_revision` (index from the rule, one undo step,
  snapshot kept; `issue_revision_as` for a typed index; `unchanged_since`
  gives "no changes since B"); batch snapshot writes in one transaction if
  issues ever come in bulk (WS-01);
- Changes mode and marks from `diff` and `changed_sheets`; show
  `ChangeSet.info.rules` more quietly than name and description (WS-03,
  WS-06); call `changed_sheets` when an issue is made, not per frame;
- title block fields from `PageRevisions` and `Project::latest_revision`
  (canvas and WS-09 PDF);
- the app confirms before deleting a frozen drawn page, and passes
  `rules.wants_generated` into `default_generated_pages_with` on open
  (WS-01).
