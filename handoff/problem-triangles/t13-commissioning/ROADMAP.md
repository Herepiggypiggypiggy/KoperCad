# T13 Commissioning triangles: roadmap

Lane of the problem-triangles workstream (docs/design/problem-triangles.md section 11). Area file: `../sites/commissioning.md`.

| Step | State |
|---|---|
| Flag builder for punch items, failed checks, open redlines (`app/commissioning_flags.rs`) | done |
| Triangles on punch rows, check rows, redline rows, schematic marks | done |
| Job flags (export, import, photo, report, test sheet, redlines apply, data not read) with Retry and a strip in the panel | done |
| Defects card: coloured sentences became triangle rows; save failure is a row | done |
| Census `problem_census_commissioning` at 0 | done |
| QA 9530-9539 at both sizes | see PROGRESS.md |
| Plate and 3D surfaces for Device anchors (T1, T3) | other lanes; commissioning flags already carry Device anchors |
| Flags list / top bar pill counting these flags | not here; commissioning flags are in the store but the pill still counts Issues only |
