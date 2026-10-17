# Schematic menus and toolbars: roadmap

Scope: menu bar (except View, owned by #739), schematic dock, right-click menus on the schematic canvas (empty, device, wire, terminal, page). Every command in at most two clicks plus Ctrl+K, grouped, shortcuts shown.

Done
- PR 1: menu bar regroup (File, Edit, Draw, Check, Reports, Simulate, Help) via `BLOCKS`, `SUB_FIRST`, `REGROUPED` in `novo-workflow/src/menus.rs`.

Next
- PR 2: right-click menus (shortcut text from the keymap, Page submenu, device/wire/terminal rows).
- PR 3: dock toolbar (Cable tool, undo/redo).
