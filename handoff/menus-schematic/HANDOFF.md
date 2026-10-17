# Schematic menus and toolbars: handoff

Branch `claude/project-thread-nimiuh` (PR 1). Menu order lives in `menus.rs`: `BLOCKS` (plain rows per menu, separators between blocks), `SUB_FIRST` (submenu order), `REGROUPED` (placements that differ from the group default; a separate table so it never conflicts with `PLACEMENTS` edits in #732/#734/#736/#739). `menu_bar.rs` draws `menus::drawn`; Learn anchors still read `menus::layout` (anchors are by command id, so ordering does not matter).
Do not touch View rows (#739). Export submenu is #734's.
Resume: `cargo test -p novo-workflow menus`, `cargo test -p novo-app`.

# Schematic menus, shortcut sheet, Ctrl+K: handoff

PRs: #749 menu bar, #766 right-click menus, #770 dock (all READY, merge lead merges). Ctrl+K polish PR: branch claude/project-thread-nimiuh-shortcuts.
Menu order lives in `novo-workflow/src/menus.rs` (`BLOCKS`, `SUB_FIRST`, `REGROUPED`, `drawn()`); the command field lists by menu using `menus::layout` and shows `palette::where_it_lives` (path like `File › Import`). After #749 merges, switch `Palette::menu` from `menus::layout` to `menus::drawn` so the empty field follows the bar's block order.
The shortcut sheet (`?`) already exists: `app/shortcuts_sheet.rs`.
