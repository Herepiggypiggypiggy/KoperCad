# Schematic menus, shortcut sheet, Ctrl+K: handoff

PRs: #749 menu bar, #766 right-click menus, #770 dock (all READY, merge lead merges). Ctrl+K polish PR: branch claude/project-thread-nimiuh-shortcuts.
Menu order lives in `novo-workflow/src/menus.rs` (`BLOCKS`, `SUB_FIRST`, `REGROUPED`, `drawn()`); the command field lists by menu using `menus::layout` and shows `palette::where_it_lives` (path like `File › Import`). After #749 merges, switch `Palette::menu` from `menus::layout` to `menus::drawn` so the empty field follows the bar's block order.
The shortcut sheet (`?`) already exists: `app/shortcuts_sheet.rs`.
