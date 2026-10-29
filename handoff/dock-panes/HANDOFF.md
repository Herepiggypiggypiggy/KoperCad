# Dock the small cards and Pages in the panel strips (V1c, V1d)

#1717 (V1c) merged: Vents and fans, Controller, Changes, EMC, Frame statistics dockable; Pages is a pane.
V1d (branch claude/project-thread-mxydup, restarted from main after #1717): EMC legend, EMC net classes, Live potentials card dockable; Reports list is a pane (`panel_dock::REPORTS`, `navigator_dock_button`, drawn in app.rs after `spacers`, `PaneOptions { scrolls: true }`).

## How it works
- `Panel::dockable()` works for `.fit()` cards. Docked panes: `panel_dock::show_docked` with `PaneOptions`.
- Pages and the Reports list are columns (egui::Panel) that become panes when `slot_of(PAGES|REPORTS)` is set; header button beside the side arrow docks them, pane header floats them back; Reset panel layout floats all.
- Changes card stands under the live card only while the live card floats.

## Tests / QA
`panel_windows_tests::the_pages_column_docks...`, `the_reports_list_docks...`, `panel_dock::tests::a_card_that_fits...`; scenarios 1132-1136.

## Not done
- No per-card QA scenario for Controller, Vents, Changes, EMC, Stats, EMC legend, EMC net classes, Live card (need setup).
- Reports pane shows its own "Reports" section label under the pane header (redundant, harmless).
