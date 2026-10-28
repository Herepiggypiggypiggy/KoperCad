# Movable panels roadmap

Goal: every panel, inspector, dialog and floating card is roomy by default, moves by its title bar, resizes, remembers where it was, and goes back with View > Reset panel layout. One helper: `crates/novo-app/src/app/panel.rs`.

1. Helper, dialogs, cabinet inspector (#712, merged).
2. Schematic inspector, wire panel, lesson card, 3D toolbar and statistics (#742, merged).
3. Changes card, live potentials card, walkthrough, heat legends (#837, merged). Follow-up: no doubled Fast keys heading, legend tour anchor, stack clamp.
4. 3D heat panel (#871, merged).
5. Everything else: the navigator, tool dock, 35 modal dialogs, raw windows and the 3D cards (this PR). Hover tips, toasts and the radial wheel stay Areas on purpose (listed in `panel_windows_tests.rs`).
