# Migration and interop: handoff

Branch: `claude/project-thread-6oep4z` (repo kopercad). Read `docs/design/interop-migration.md` first.

State: first slice implemented and tested (see ROADMAP.md "Done"). PR number and head SHA are in PROGRESS.md.

Resume:
- `cargo test -p novo-interop -p novo-cli -p novo-app`, then `cargo clippy --workspace --all-targets -- -D warnings`
- STEP check outside the repo: `pip install cadquery-ocp`, read the file with `STEPControl_Reader`, check `BRepCheck_Analyzer`, bounding box, solids.
- Example writer: `cargo run -p novo-interop --example step_cabinet -- out.stp`

Decisions:
- DWG is never read by KoperCAD; a user-installed free converter runs as a separate process (licences).
- Imports only add and preview first; every engine returns a `Report` with the lossy parts.
- The enclosure is exported as a 2 mm housing with open front, not a solid block.
- AutomationML nets are elements with one interface per pin; wires are not recreated on import (needs pages).

Traps:
- `novo-view3d` gained `profile_boxes` (small, `mesh.rs`); keep `profile_mesh` using it.
- `eplan::xml` is `pub(crate)` now (the AML reader reuses it).
- No Windows hands-on test of the DWG converter paths; the process driver is tested with stand-in scripts on Linux only.
