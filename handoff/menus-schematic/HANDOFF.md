# Schematic menus and toolbars: handoff

Branch `claude/project-thread-nimiuh` (PR 1). Menu order lives in `menus.rs`: `BLOCKS` (plain rows per menu, separators between blocks), `SUB_FIRST` (submenu order), `REGROUPED` (placements that differ from the group default; a separate table so it never conflicts with `PLACEMENTS` edits in #732/#734/#736/#739). `menu_bar.rs` draws `menus::drawn`; Learn anchors still read `menus::layout` (anchors are by command id, so ordering does not matter).
Do not touch View rows (#739). Export submenu is #734's.
Resume: `cargo test -p novo-workflow menus`, `cargo test -p novo-app`.
