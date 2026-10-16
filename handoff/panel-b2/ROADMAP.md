# PANEL-B2 roadmap: door swing, openings, per-surface place-all

1. Done (slice 1): `novo-layout/src/swing.rs` door swing sweep (devices on the door against plate devices and walls, first touch angle, rule names `door.swing` / `door.closed`), `CabinetLayout::door_devices` and `swing_report()`, lines in the Cabinet Door and depth panel.
2. Next: wiring-hole Opening keep-outs and pass-through between plates (drill.rs, for routing).
3. Next: Place all per surface with rule sections (surface door), door devices with hole and cut-out work (R5).
4. Next: fill `door_devices` from placements whose `surface` property is `door`; hinge offset as a project setting.
