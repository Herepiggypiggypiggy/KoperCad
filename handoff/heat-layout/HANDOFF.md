# Heat-optimised layout: handoff

## What exists
- `novo-thermal/src/placement.rs`: rail-aware search for a cooler layout. Moves only along free gaps on rails or swaps, ranks devices by plume exposure, cost is the sum of shortfalls to the safe margin (8 K band) plus a wire price (0.5 per metre) and a largest move (900 mm). Returns the moves, before/after numbers, and the cooling that was tried when a device stays over.
- `novo-thermal/src/placement_project.rs`: reads the project into rows and applies the moves as one undo step.
- `novo-thermal/src/lump.rs`: merges runs of cold neighbours into blocks (helper for the 3D mesh; not wired to the Simulate button yet).
- App: Cabinet, Heat strip, button "Cooler layout" (also Ctrl+K "Suggest a cooler layout" and the plate right-click menu). Window with before/after, arrows on the plate, Apply the moves, Ctrl+Z undoes.
- Tests: `crates/novo-thermal/tests/placement.rs`, `heat_layout_tests.rs`, QA scenario `qa/scenarios/14-cooler-layout.qa` with `qa/fixtures/cabinet-cooler-layout.novo`.

## What it is not
- The search uses the 2D model with planning values. Not validated (`ENCLOSURE_VALIDATED` stays false). It does not know the wiring beyond distance.
- The 3D Simulate button does not yet cope with a real machine cabinet (see findings): the lumping helper is the first step.

## Where to read more
`audit/real-machine/heat-layout-findings.md` (project files), `ROADMAP.md` for the next lanes.
