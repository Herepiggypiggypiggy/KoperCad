2026-10-21 10:00 UTC Thread started from the round 5 audit (no revision lock). Found issue, snapshot, compare and revision table already on main.
2026-10-21 10:40 UTC Added page lock in novo-core, issue_revision_locked and restore in novo-revisions with tests.
2026-10-21 11:00 UTC App: Issue box, Lock/Unlock/Restore commands, Compare window Restore buttons.
2026-10-21 10:30 UTC PR #1167 opened. R3 review: restore now asks first (counts, Esc/click-outside cancels), buttons offered for whichever side is a revision, lock toast says "since X", error colour from ui_palette. Default-on lock kept (decision for Tadjiro if unwanted).
2026-10-21 10:45 UTC R-review notes: LockFailed error (revision issued but lock failed), UI text says reports and parts are not locked, release note line, test that devices can be added elsewhere while pages are locked.
2026-10-21 10:55 UTC Failed lock after issue now keeps the snapshot and returns the Issued (LockFailed carries it); test in issue.rs.
2026-10-21 11:50 UTC C-30: Issue revision needs a description or the No description box.
2026-10-21 12:20 UTC Lock marks: outline icon, status chip, contents columns Issued and Locked (PR from main).
2026-10-21 12:50 UTC #1181: QA scenario 09-revision-lock, padlock no longer overlaps long titles, unlock message says Undo locks it again. Screens in /mnt/project-files/qa/revision-marks/.
2026-10-21 12:45 UTC follow-up: padlock tooltip explains generated pages keep regenerating (R3 ask).
