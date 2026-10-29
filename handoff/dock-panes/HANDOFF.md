# Dock the small cards and Pages in the panel strips (V1c)

Branch `claude/project-thread-mxydup` (kopercad). Code head c151f0c8; QA in `qa/pr-v1c-c151f0c8/`.

## Done
- `Panel::dockable()` now works for cards that `.fit()` their content (they float at their own size, fill the strip once docked). Dockable: Vents and fans (`view3d_vents.rs`), Controller (`controller_card.rs`), Changes (`changes.rs`), EMC panel (`emc_view.rs`), Frame statistics (`view3d_stats.rs`).
- Pages is a pane: `panel_dock::PAGES`, button beside the pin in the Pages column header docks it into the strip on its side (`pages_dock_button`); drawn in `app.rs` right after `panel_dock::spacers` via `show_docked` with `PaneOptions { scrolls: false }`; the pane header sends it to another strip or back to the column. Only when the column is pinned and not the Commissioning tablet view. Reset panel layout brings it home.
- `show_docked` takes `PaneOptions` (closable, float_tip, scrolls).
- Tests: `panel_dock::tests::a_card_that_fits_its_content_docks_and_floats_again`, `panel_windows_tests::the_pages_column_docks_as_a_pane_of_a_strip_with_other_panels`; scenarios 1135 (both sizes), 1134 click moved (side arrow is now left of the dock button).

## Not done / next
- Other fit cards (EMC legend, schematic EMC net classes, live card, selection sheet) are not dockable yet: add `.dockable()`.
- Reports list column is still side-switch only.
- No per-card QA scenario (controller/vents need setup); covered by the generic fit test.
- QA failures at 1366x768 seen in the broad "ock" filter (1603 flyout selection count, 24/25 Place Button, 5620 add-on trust) were not checked against main.
