# UI small fixes (3D refit, Export titles)

- 3D camera refit: `app.rs` sets `view3d.pending = Fit` when `panel_dock::take_changed` fires, so docking, moving or floating a panel refits the 3D view as it already did the schematic.
- Export tile titles at 1920x1080: could not reproduce on main f976c8e. Every tile in all sections (also with a format picked) shows its whole title. Scenario 7380 scrolls the whole list as a regression guard; look at its pictures. If Tadjiro still sees a cut title, ask for the tile name.
- Scenarios: 7380 export-tile-titles, 7381 3d-refit-after-dock (both run at 1366x768 and 1920x1080) (opens the Navigator over 3D; it does not drag-dock).
