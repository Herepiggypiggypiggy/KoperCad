# UI small fixes (3D refit, Export titles)

- 3D camera refit: `app.rs` sets `view3d.pending = Fit` when `panel_dock::take_changed` fires, so docking, moving or floating a panel refits the 3D view as it already did the schematic.
- Export tile titles at 1920x1080: could not reproduce on main f976c8e. Every tile in all sections (also with a format picked) shows its whole title. Scenario 7380 scrolls the whole list as a regression guard; look at its pictures. If Tadjiro still sees a cut title, ask for the tile name.
- Scenarios: 7380 export-tile-titles, 7381 3d-refit-after-dock (both run at 1366x768 and 1920x1080) (opens the Navigator over 3D; it does not drag-dock).

## 3D toolbar and view cube (follow-up PR)
At 1366x768 with the Navigator open the 3D toolbar ran under the view cube: its width ignored the room the Navigator takes. `view3d_controls::toolbar` now subtracts `panel::flyout_width`. Scenario 7382 (both sizes).
Drag-dock scenario for the refit: not added. The Navigator is a flyout, dragging its title only moves it and does not change the canvas size, so it cannot exercise the refit; a dockable panel in the 3D workspace is needed for that.
