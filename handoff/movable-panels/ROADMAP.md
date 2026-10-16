# Movable panels roadmap

Goal: every panel, inspector, dialog and floating card is roomy by default, moves by its title bar, resizes, remembers where it was, and goes back with View > Reset panel layout. One helper: `crates/novo-app/src/app/panel.rs`.

1. Helper, dialogs, cabinet inspector (#712, merged).
2. Schematic inspector, wire panel, lesson card, 3D toolbar and statistics (#742, merged).
3. Changes card, live potentials card, walkthrough, heat legends (#837, merged). Follow-up: no doubled Fast keys heading, legend tour anchor, stack clamp.
4. Left: the 3D heat panel (`view3d_heat.rs`, an Area tied to the view cube), the outline flyout (design lead), tool dock, small tooltips (stay Areas on purpose).
