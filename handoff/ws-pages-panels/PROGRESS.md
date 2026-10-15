# Progress
- 2026-10-15: model and tests (panels, page groups, SetPageStructure). Navigator UI next.
- 2026-10-15 12:45 UTC: PR #241 (model) merged. Navigator UI built: New menu (Pages/Page group/Panel form), Function and Location views, group/panel headers with "same layout" marker, right-click New/Move, drag a page onto a header to move it.
- 2026-10-15 13:15 UTC: #241 and #248 merged. Recent pages list removed from the outline (Tadjiro's request), new PR.
- 2026-10-15 15:45 UTC: #248 and #257 merged. Contents list now has Page group and Panel columns plus a Panels section (panel, groups, page count). Next: inline page rename (RenamePage). Drag-reorder and group/panel rename deferred: page names are references and device tags carry the same location, so they need a design call.
- 2026-10-15 16:10 UTC: #303 merged. Inline page rename (RenamePage command, double-click a page row in the outline).
- 2026-10-15 17:10 UTC: lead asked for DESIGN proposal: group/panel rename (RenameLevel), drag-reorder (order property), per-panel contents page. Doc: docs/design/pages-rename.md (PR open, awaiting approval).
- 2026-10-15 17:30 UTC: #321 merged. RenameLevel command in novo-core (tests/rename_level.rs), PR open. Next: outline rename on group/panel rows, then ReorderPages + order, then per-panel contents page.
