# Enclosure depth and door swing: handoff

Updated 2026-10-15.

- Branch `claude/project-thread-tv09ns`, stacked on #374, #376 and #378 (the
  Cabinet canvas edits); retarget to main when they merge.
- Engine: `crates/novo-layout/src/enclosure.rs` (types, `check`, swing, arc).
  Storage: `novo_layout::model::{enclosure_of, set_enclosure, device_depth}`
  as Structure properties (keys in `model::keys`), no novo-core change.
  `CabinetLayout` has `enclosure`, `depths` and `door_report()`.
- UI: `crates/novo-app/src/app/cabinet_door.rs`, right panel of the Cabinet
  canvas; problems count in the header strip includes the door checks. Front
  view boxes are not coloured for depth (that needs a `BoxState` in novo-canvas).
- Not hands-on tested in a running window (cloud session).
- Design: docs/design/cabinet-layout.md section 5b.
