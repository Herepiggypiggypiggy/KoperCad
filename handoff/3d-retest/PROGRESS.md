# 3D re-test as a first-time user (away-week policy)

- QA scenarios: `qa/scenarios/40-3d-walk.qa` (open, layers, views, dimensions, section, door), `41-3d-select-clearance.qa` (front view, select K1, dimensions and dashed clearance zone, 1366 only), `42-3d-heat.qa` (3D Heat panel, Simulate, verdict, hot-spot labels). Pictures in `/mnt/project-files/qa/3d-retest/`.
- Found and fixed: panels (`panel.rs`) never followed their canvas once it moved. egui ignores the position given to a title-bar window after its first frame, so with the Heat strip appearing above the 3D view the Heat panel stayed put and covered the view cube and the toolbar sat over the strip. A panel that is not dragged now takes its start again (drag-from-anywhere for that one frame, not while a button is down); a click that holds on a panel no longer counts as a drag (the QA harness hold made it "moved" and pinned it). Tests: `an_unmoved_panel_follows_its_canvas_when_the_canvas_moves_down`.
- Not mine: the first-show fit clear of the toolbar landed as #1191 from the UX sweep thread.
- Seen, not fixed: the open door swings below the view at 1920 and the fit does not include it; the empty strip at the bottom of the 3D Heat panel; the Heat colour scale max changes during a run.
- Next: cable runs in 3D (wires follow duct and rail routes, length per run), a duct fill picture, the unfinished tour button.
