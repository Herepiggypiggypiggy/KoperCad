# Cabinet lessons: roadmap

Scope: the lessons of the `cabinet` track (panel building), written as `assets/lessons/<id>/lesson.toml`
plus sample projects, against `docs/design/learning-modules.md`.

## Done (draft PR)
- Nine lessons, all `status = "draft"`: cabinet-layout (10), rails-and-ducts (20), wire-routing (30),
  cut-lengths (40), enclosure-door-and-depth (50), heat-and-sizing (60), panel-plate-drawing (70),
  panel-labels (80), view-3d (90).
- Sample projects for the first two lessons; the rest share them (three states, see HANDOFF).
- Glossary terms for the track; Dutch twins for titles, summaries and glossary.
- Generator: `crates/novo-app/examples/panel_lessons.rs`.

## Next
1. Flip a lesson to `ready` in its own PR when every control it names has an anchor and its steps are on main
   (table in HANDOFF). First candidate: cabinet-layout, once the Cabinet registers its anchors and signals.
2. Add `finished.json` for enclosure-door-and-depth when #409 merges (the inside size is stored in a form that
   branch defines), and re-check the depth numbers in the lesson.
3. Dutch twins for the step texts (84 steps and titles are counted by the lint).
4. `commissioning` (cabinet track in the design's list) once #471 merges: not written, it was not in the brief.

## Next (2026-10-22)
- A second, richer cabinet lesson project (PLC, 24 V supply, drive, relays, several terminal strips) with a lesson that places it, reads the heat and checks the 3D view: more complexity down the track, asked by Tadjiro. New lesson directory, appended to `paths.toml`; the Learning and example coverage thread owns progression.
- A design check for a device recorded on a rail whose line does not pass through its centre (plans saved before #1356 keep the rail on the top edge until Place all is run again).

## Open questions
- State checks for the cabinet (`placed`, `rail`, `duct`) and cabinet `solve` ops: asked of the framework thread.
