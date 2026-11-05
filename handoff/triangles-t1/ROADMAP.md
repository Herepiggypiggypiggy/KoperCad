# Lane T1: Cabinet plate flags

Scope (docs/design/problem-triangles.md, LANE-PROMPTS.md T1): the physical Issues kinds (DeviceCollision, DeviceOutsidePlate, DevicesTooClose, RailOverflow, DeviceOffRail, FieldDeviceInCabinet, DuctOverlap, OverfullDuct, UnsizedDuct) are triangles on the Cabinet plate; the Cabinet keeps no problem list of its own.

Done in this PR
- `app/cabinet_flags.rs`: plate kinds, `plate_flags`, `roll_up`, `device_flags`, `clusters`, `draw` (14 pt triangle on the top right corner of a device or the plate frame, count badge for several flags, 24 pt hit area, keyboard focus, accessible name), Layout-source flags for broken Place rules and covered wiring openings.
- Retired: `problem_lines`, `problem_summary`, `panel_problem_lines`, `rail_duct_lines`, the "N problems" popup pill and the "N rule problems" pill. The Cabinet pill is the top bar's triangle pill filtered to the plate; click opens the same list.
- Table "Problems" column, device hover and the inspector section read the store (triangle plus numbers).
- `novo-checks`: OverfullDuct and UnsizedDuct also name the plate (Structure) so the Cabinet can draw them.

Next (other lanes)
- T2 door and depth: the door kinds already reach the plate frame and devices through the same store; T2 adds the arc, enclosure and 3D, and removes the remaining sentences.
- T3 3D painter: the plate painter here is egui-only; the 3D one is T3's.
- Layout-source flags are not in the Issues list rows or the top bar count yet (Flags list promotion).
