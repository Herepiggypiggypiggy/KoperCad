# Review notes: roadmap

Scope: comments on the project, pages, objects and spots, with author, status (open/resolved), replies, filter, jump-to, PDF markup and handover. Stored in the project file sidecar `review_notes`.

Done (PR from branch claude/project-thread-0mqs8q):
- Engine `novo-revisions::notes` (ReviewNotes, locate, filter, pdf markers, printed list, handover table).
- App: Review notes panel (Check menu, Ctrl+K "notes"), right-click "Add review note...", bubbles on canvas, status bar chip, Next open note.
- novo-pdf: PdfMarker bubbles and extra appendix; PDF options checkbox; handover `10 Review notes.pdf`.
- Tests, QA scenarios 190-193, manual section in docs/manual/handover.md.

Next (not built):
- Notes in the cabinet and 3D workspaces (panel shows only on the schematic).
- Per-user names (author is the revision initials).
- Notes in the Excel handover workbook; import/merge notes from a reviewer's copy of the file.
- Key binding for notes.add.
