# Chip docking: handoff

Read INVENTORY.md first: every floating chip, where it was, where it goes.
Rule: state goes to the status bar (`status_bar.rs`: `ModeChip` slots, `sim_chip::bar_slot`); controls go into a dockable pane (`panel_dock.rs`).
The test `no_fixed_window_is_built_outside_the_panel_helpers` (`panel_windows_tests.rs`) lists the raw egui Areas that remain; remove entries as items move.
Do not stack on #1807 / #1808; rebase after they merge.
