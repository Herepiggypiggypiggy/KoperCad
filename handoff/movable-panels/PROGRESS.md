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
