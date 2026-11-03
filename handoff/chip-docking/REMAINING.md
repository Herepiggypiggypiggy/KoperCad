# Chip docking: what still floats over the canvas (after the 3D pane PR)

Audit 2026-11-03 of `egui::Area`, `Float`, `Panel` without dock in `crates/novo-app/src/app` (guard test: `no_fixed_window_is_built_outside_the_panel_helpers`).

## Docked or in the status bar now
Simulation status, injected-defect warning, Fast keys (status bar, #1811). 3D view pane (Views, Section, door, quality warning), Place, Measure, Position, Align and spread, Wire it, Build order, Mouse and keys, Build mode "away" strip: panes of the right strip, foldable, floatable. "? Mouse and keys" chip removed; the 3D rail has a "3D hints" row.

## Still floating, on purpose or not yet done
| Item | Where | Why / next step |
| --- | --- | --- |
| Lesson card | bottom right (bottom left in Cabinet) | A guide that appears only while a lesson runs; lesson steps point at it by anchor and tests read its corner. Next: titled, dockable pane. Not done here |
| Walk-through card (Fast keys list, no Live) | bottom right | Same kind of card; has a Skip/Close. Next: dockable |
| Heat legend (Δ compare) | 3D | Already dockable (title bar) |
| Narrow windows (under 1100 wide) | 3D view pane | Floats at the top left instead of a 320 wide strip; dock it by hand |
| Toasts, hover tips (device tip, wire hover, drag gaps, live wire tip), move warning, command field, right-click menus, place wheel, empty-sheet hint | transient | Appear for a moment at the pointer or fade; not chips that stay |
| Live badges (pole, coil, input, load) | on the sheet | Part of the drawing |
| Cursor readout (HUD) | top right | Only when the status bar is switched off |
