# Movable panels progress

- 2026-10-16: #712 merged (helper, 19 dialogs, cabinet inspector).
- 2026-10-16: #742 merged (inspector, wire panel, lesson card, 3D toolbar and statistics).
- 2026-10-16: panels 3 open (changes card, live card, walkthrough, heat legends).
- 2026-10-28: all windows migrated (this PR). New in `panel.rs`: `modal` (what `egui::Modal` was, now with a title bar to drag and edges to resize; 35 dialogs), `Float` (cards without a title bar: tool dock, 3D place/measure/position/align/wire-it/build-order cards, build-mode away bar), `Panel::is_flyout` (the navigator flyout is a window), resizable edges for the pinned navigator, report list, drawer and the detail columns (Reset panel layout clears them). The raw windows (Cooler layout, Cooling advice, Cooling what-if, Photo, Properties of the selection) and the multi-selection inspector go through `dialog`/`Panel`. Guard test `no_fixed_window_is_built_outside_the_panel_helpers` lists the few raw Areas that stay (hover tips, toasts, the wheel, badges on the sheet) with the reason.

## Verdicts for the `resizable(false)` / `collapsible(false)` sites (visual-redesign.md section 8, lane V1)

| Site | Verdict |
| --- | --- |
| `app.rs` outline column | now resizable (200 to 520 pt, default 248); the Commissioning tablet column stays fixed (touch layout) |
| `app.rs` `reports_list` | now resizable |
| `parts.rs` `parts_detail` / `cabinet.rs` / `parts_quality.rs` x3 / `cabinet_routes.rs` / `drawers.rs` | now resizable side panels with a minimum and a maximum |
| `heat_layout.rs`, `heat_sweep.rs`, `heat_whatif.rs`, `commissioning_viewer.rs` | raw windows replaced by `panel::dialog`: drag, resize, remembered |
| `parts.rs` `parts_status` strip, `assistant.rs` agree strip | stay fixed: one-line strips inside a window that is itself resizable |
| `terminal_edit.rs`, `learn_suite.rs`, `panel.rs` `modal` `collapsible(false)` | stay: question dialogs and the lesson window do not roll up; they move and resize |

Not in this PR (lane V1b): dock slots with snap zones, double click maximise, dock left/right buttons in the header, corner grip, splitter double click reset.

## V1b-1 (maximise and splitter reset)

- 2026-10-29: double click on a window's title bar maximises it (a `Panel` to its canvas, a `dialog` or `modal` to the window) and a second one restores it. `panel::maximise_step` reads egui's `__title_click` response, keeps the restore rect per window and per Reset-layout epoch, undoes egui's own fold-on-double-click for collapsible windows (the arrow still folds), and lifts the window's `constrain_to` and `max_height` for the maximised state. Double click on the edge of an `EDGE_PANELS` panel removes its `PanelState` (`panel::splitters_reset`, called once per frame from `app.rs`). Tests in `panel_windows_tests.rs`; QA scenarios 1110, 1111, 1112. Next: V1b-2 dock slots, snap zones and dock buttons; V1b-3 Pages and Reports as docked panels and the sim chip corner.

## V1b-2 (dock slots, snap zones, dock buttons)

- 2026-10-29: new `panel_dock.rs`. A `Panel` built with `.dockable()` (not for `.fit()`, `.flyout()` or title-less cards) gets two dock buttons painted on its title bar and snap zones (72 points at the canvas left, right and bottom edge) while its title bar is dragged. A panel with a slot is drawn by `show_docked` as a Background `Area` (`<panel id>/docked`) inside the strip that `spacers` reserve with a side panel at each edge (`dock_left`, `dock_right`, `dock_bottom`, in `EDGE_PANELS`, so the splitter reset applies). Several panels in one slot share it equally. The slot of each panel persists in egui data under `panel_dock`; View > Reset panel layout calls `panel_dock::reset`. Dockable now: Inspector (also wire card), cabinet Inspector, 3D Heat, wire runs, Review notes. The `.fit()` cards (Vents and fans, Controller, Changes, EMC, Stats) stay floating until V1b-3 decides how they dock. Tests in `panel_dock.rs`; QA scenarios 1120, 1121, 1122.
- Known: docking does not refit the camera, so the sheet can be partly under the strip until Home (zoom to fit); a refit on a dock change is a V1b-3 candidate.

## V1b-3 (refit, divider, navigator side, hint)
- 2026-10-29: `panel_dock::take_changed` gives one refit signal a few frames after a panel changes slot (or Reset layout floats docked panels), and `app.rs` calls `canvas.zoom_to_fit()` on it, so the sheet is not left under a strip. Several panes in one slot now have weights (`dock_weights`, persisted per slot): a 6 px grip at the foot of a pane (right edge in the bottom slot) is dragged to move share between neighbours (each keeps `PANE_MIN_HEIGHT` / `PANE_MIN_WIDTH`), a double click equalises the pair; weights are equal again when the pane count changes. The Pages column and the Reports list sit left or right (`panel_dock::navigator_right`, persisted `navigator_right`; panels `outline_right` and `reports_list_right`, both in `EDGE_PANELS`); the button is `navigator_side_button` in the pinned column header and the Reports list header; Reset layout sends the column home. `panel::title_hint` shows "Double click to maximise" or "Double click to restore" after the pointer rests 0.5 s on a title bar outside the button zone. Tests in `panel_dock.rs` and `panel_windows_tests.rs`; QA scenarios 1130, 1131 (hint), 1132, 1133 (divider), 1134 (column side).
- Not in this PR: the `.fit()` cards (Vents and fans, Controller, Changes, EMC, Stats) still only float; Pages as a free docked panel (it is a column that moves between the two sides, not a pane in the dock strips); clearance for the sim status chip in the 3D and Cabinet corner (docking does not move that corner, the chip sits at the canvas top left and a left strip moves the canvas with it).

