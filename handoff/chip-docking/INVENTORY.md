# Floating chips, pills, strips and badges: inventory and where each goes

Written 2026-11-02 for Tadjiro's request "no undockable chips; they clutter the screen".
Source: sweep of `crates/novo-app/src/app` on main f90872a (before #1807 docking and #1808 3D rail merge) for `egui::Area`, `Float::new`, `Panel::...bare()` and `Panel` without a title bar.

Answer to the question: no. Today several chips float over the canvas and cannot be docked or folded. #1807 docks windows and cards that have a title bar, and says so in its own list: "No title bar, so nothing to dock (small tool strips and chips)". This change covers that gap.

Rule used: a thing that reports a state goes to the **status bar** (already docked, one row, always in sight). A thing that holds controls goes into a **dockable, foldable pane** or the rail flyout. Drawing on the sheet itself (live current dots, wire hover labels) is part of the drawing, not a chip, and stays.

## A. Floating chips and strips to remove

| # | Item | Where now | File | Goes to |
| --- | --- | --- | --- | --- |
| 1 | Simulation status chip (Run / Cancel / Show heat, progress) | Cabinet top left card; first row of the 3D toolbar | `sim_chip.rs`, `cabinet.rs`, `view3d.rs` | Status bar slot, left of the mode chips, in Cabinet and 3D. Click runs the one fitting action; hover has the detail line |
| 2 | 3D toolbar slim strip: Views, door open slider, quality warning, Section controls | 3D top left, bare Panel | `view3d_controls.rs` | One dockable **3D view** pane (Views, Door, Section, Quality). Quality warning also goes to the status bar |
| 3 | Build order strip | 3D bottom centre | `view3d_build_order.rs` | Docked pane **Build order** (right strip), opens when the mode is on |
| 4 | Wire-it strip | 3D bottom centre | `view3d_wire_it.rs` | Docked pane **Wire it** |
| 5 | Align strip | 3D bottom centre | `view3d_align.rs` | Docked pane **Align** (shares the 3D tools pane) |
| 6 | Nudge strip | 3D bottom right | `view3d_nudge.rs` | Same 3D tools pane, **Nudge** section |
| 7 | Measure strip | 3D bottom left | `view3d_measure.rs` | Same 3D tools pane, **Measure** section; the reading also in the status bar |
| 8 | Place strip | 3D top left | `view3d_place.rs` | Same 3D tools pane, **Place** section |
| 9 | 3D hints strip | 3D bottom right | `view3d_hints.rs` | Status bar hint (the same slot the schematic tool hint uses) |
| 10 | Duplicate strip | 3D | `view3d_duplicate.rs` | 3D tools pane, **Duplicate** section |
| 11 | Move warning | 3D, `view3d.move_warning` area | `view3d_move.rs` | Status bar refusal message / toast (already exist) |
| 12 | Walkthrough "Fast keys N of M" pill | Bottom right when Live is on | `onboarding.rs` | Status bar slot while the walkthrough runs; full card stays a dockable pane |
| 13 | Defect banner "DEFECT INJECTED (n)" | Top centre of the canvas | `defects_card.rs` | Status bar mode chip (red), click opens the Defects card |
| 14 | Lesson card | Floating bare Panel | `learn_ui.rs` | Dockable pane (title bar added; it already has the content) |

## B. Already docked or part of the drawing: stay

Status bar mode chips (Marks hidden, References, Live, machine options), cursor readout, saved state.
Live view badges (pole, coil, PLC input dots, load glow): painted at symbol positions, they are the drawing. Not chips.
Transient: toasts, hover tips (device tip, wire hover, drag-gap labels), command field, right-click menus, place wheel, empty-sheet hint, HUD (only when the status bar is hidden; then it is the only readout, kept).

## C. Order of work (PR from main, not stacked)

1. Status bar slot for the simulation status (#1) and the defect banner (#13), walkthrough pill (#12). Remove `sim_chip::show` from the Cabinet, and the first row of the 3D toolbar.
2. 3D view pane (#2) and 3D tools pane (#3 to #10, #11) after #1807 and #1808 merge, because both rewrite `view3d_controls.rs` and the panel dock.
3. Lesson card title bar (#14).
Tests: a source scan that fails when a new bare floating Panel or `Float::new` is added without being listed here; placement tests at 1366x768.
