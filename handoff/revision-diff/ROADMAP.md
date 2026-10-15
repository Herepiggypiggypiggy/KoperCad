# Revision diff on the canvas: roadmap

Scope: the compare experience on top of novo-revisions' `diff` and the kept
snapshots. The "Revision tracking and compare" thread owns snapshot storage
and the revision table; this thread owns what a person sees when comparing.

Already on main: `diff`, `changed_sheets`, Changes mode marks (green, amber,
dashed red ghosts), the outline's Changes list with per-page counts.

Done:
- `novo_revisions::change_list`: flat, named change rows (this PR).

Next, in order:
1. Change list in the app: rows from `change_list`, click selects and reveals the item.
2. Compare any two revisions (not only "since" against now).
3. Change report in novo-pdf: cover list of rows per page, with marks on the page images.
4. Cleaner overlay for moved items (old position ghost plus new outline).
