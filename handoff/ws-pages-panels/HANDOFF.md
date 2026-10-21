# Handoff: pages, groups and panels

Ask: make new pages, page groups and panels from the pages navigator. Pages in the same panel name under different page groups (`=20D41+BAY1G1`, `=21D21+BAY1G1`) are one panel, so one cabinet layout.

State: no new entity. `Page::structure` already holds group and panel. Added `Command::SetPageStructure`, `structure` on `Command::AddPage`, `Project::panels()`, `page_groups()`, `next_page_name()`. Tests: `crates/novo-core/tests/panels.rs`.
Next: outline UI (new page/group/panel, move page), coordinate with Layout and App shell threads; interruption points (Potentials thread) use page names with the structure prefix.

UI (PR 2): crates/novo-app/src/app/pages.rs (form + form_command, tested) and the outline in app.rs (structure_headers, move_page, PageView). Spec: DESIGN PR #238 section Pages navigator. Not done: Cabinet canvas per panel (Layout thread), page-name rules for inserted pages (section 3).

Rename, reorder, contents (2026-10-15, design in docs/design/pages-rename.md, all merged):
- Rename: `Command::RenameLevel { aspect, from, to }` (`Tx::rename_level`) rewrites a group or panel in pages, devices, cables, structure nodes and panel contents ids in one undo step; frozen pages keep what was printed; a clash refuses, a rename onto an existing name merges. Tests: `novo-core/tests/rename_level.rs`. UI: double-click or right-click Rename on a group or panel header (`structure_headers`, `pages::rename_level_command`).
- Reorder: `Command::ReorderPages { pages }` writes the `order` property (10, 20, ...) on the pages of one group, panel and section (`Project::sibling_pages`); `page_cmp` sorts by structure, `order` (unset last), name; names never change. Tests: `tests/reorder_pages.rs`. UI: drag a page onto another (`pages::order_after_drop`), Move up/down in the page menu.
- Contents per panel: generator id `toc@+BAY1G1` (`panel_contents_generator`, `contents_scope`, `is_contents_generator`); `novo_reports::PanelContents` is registered lazily in `LiveReports::get`. Tests: `novo-core/tests/panel_contents.rs`, `novo-reports/tests/panel_contents.rs`. UI: right-click a panel header, Add contents page (one per panel).
Open: order of groups and panels themselves (design: `order` on the `Structure` node, created on first reorder); merged panels leave two contents pages with one id; the project contents list shows a panel list as one plain row; none of the outline interactions (double-click, drag line, menus) has had a hands-on test in a real window.

Update 2026-10-15: the outline UI was rebuilt as a tree (handoff/outline-tree, docs/design/outline-tree.md). `structure_headers` and the page row code are gone; rename, reorder and contents commands are unchanged and used by the new menus.
