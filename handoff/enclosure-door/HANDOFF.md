# Enclosure depth and door swing: handoff

Updated 2026-10-15.

- Branch `claude/project-thread-tv09ns`, PR #409, based on main (the Cabinet
  canvas edit PRs have merged).
- UI entry point: open a project, switch to the Cabinet canvas, press the
  Plate chip: the inspector's plate card has the collapsible section "Inside
  and door" (form, Apply as one undo step, problem list, top view with the door
  arc). Door problems count in the header strip.
- A device too deep for the door gets `BoxState::TooDeep` (error colour and a
  down-pointing triangle mark) and a sentence in the hover, table and inspector.
- Still missing in the UI: checks as `novo-checks` Problems (they show only in
  the card, the marks and the header count), door-mounted devices. Coordinate with "UI integration: cabinet, 3D,
  enclosure".
- Engine: `crates/novo-layout/src/enclosure.rs` (types, `check`, swing, arc).
  Storage: `novo_layout::model::{enclosure_of, set_enclosure, device_depth}`
  as Structure properties (keys in `model::keys`), no novo-core change.
  `CabinetLayout` has `enclosure`, `depths` and `door_report()`.
- UI: `crates/novo-app/src/app/cabinet_door.rs` (`DoorState::show`, drawn by
  `cabinet_inspector.rs` in the plate card); problems count in the header strip includes the door checks. Front
  view boxes are not coloured for depth (that needs a `BoxState` in novo-canvas).
- Not hands-on tested in a running window (cloud session).
- Design: docs/design/cabinet-layout.md section 5c.
