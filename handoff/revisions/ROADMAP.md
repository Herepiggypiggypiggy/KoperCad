# Revision lock and history: roadmap

Source: audit/creative/round5-skeptical-buyer.md, row 7 ("no revision lock").
Wanted: issue a revision, lock released sheets, a revision table in the title block, a diff view of what changed since the last issue, and restore. SQLite project file, local only, no admin rights.

Already on main before this thread (WS-18 and follow-ups): Issue revision window, snapshots in the project file, freezes, revision table in the title block (screen and PDF), Compare window with drawing view, Changes mode, change report, CLI and Python.

Done in this thread:
- Page lock: `Page.locked`, `Command::LockPage/UnlockPage`, enforced for every edit path in `Project::transact`.
- `issue_revision_locked`: issue and lock all pages in one undo step; the snapshot holds the locks.
- `novo_revisions::restore`: page or whole project back to a snapshot, one undo step, history untouched.
- App: "Lock the released pages" box in Issue revision, File > Revisions > Lock page / Unlock page / Restore page to revision, Restore buttons in the Compare window.

Next, in order:
1. Lock indicator in the outline and on the page tab.
2. CLI `kopercad-cli revisions restore` and `lock`; Python calls.
3. Lock edits to project info (name, description) once a sheet is locked; today SetInfo is not guarded.
4. Per-page "released" selection (lock only some pages at issue).

Open questions: should locked pages also refuse device-list edits that only change data not drawn on the page (today a device edit is refused only if a symbol of it is on a locked page).
