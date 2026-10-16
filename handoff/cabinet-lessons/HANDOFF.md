# Cabinet lessons: handoff

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-i1mh6g` (restart from main after each merge).
- One draft PR with nine draft lessons under `assets/lessons/` (cabinet track), glossary rows, the generator
  example and these handoff files. Checked against the lint of #479 (novo-learn, not on main yet): no errors;
  the warnings are the anchors, signals and commands that do not exist yet.

## Resume
```
git fetch origin && git checkout claude/project-thread-i1mh6g
cargo run -p novo-app --example panel_lessons -- assets/lessons   # rewrites the three project files
cargo test -p novo-learn                                          # once #479 is on main
```
To check a lesson before #479 merges: `git fetch origin pull/479/head:pr479`, merge it on a scratch branch and run the tests.

## Sample projects (three states of one cabinet)
| File | State | Used by |
| --- | --- | --- |
| `cabinet-layout/project.json` | Motor starter, cabinet `+CAB` with a 600 x 800 mm plate, nothing placed | cabinet-layout |
| `cabinet-layout/finished.json` | Place all with the standard rules: 8 devices on 5 rails, 5 ducts (3 power, 2 signal) | start of rails-and-ducts |
| `wire-routing/project.json` | Same plus one 25 x 60 duct down the right-hand end that joins the five ducts | start of wire-routing and the lessons after it |

Parts carry width, height, depth and power loss; `-T1` (transformer, not on the schematic) has a 4-hole pattern and
20 W; `-S0` and `-S1` have a 22 mm door cut-out; wires are numbered. Wire list lengths: unplaced = estimate,
Place all alone = all 11 wires between rows in free air (its ducts are not joined: reported to the lesson-plan thread); with the joining duct = routed, fullest duct 10 %, longest wire 1924 mm.
Layout facts: 8 rail holes of 5.5 mm + 4 of 6 mm on the plate, 2 door cut-outs.

## What is not on main (a lesson stays `draft` until its row is clear)
| Lesson | Needs |
| --- | --- |
| cabinet-layout | Anchors and signals below only; all features are on main |
| rails-and-ducts | Ducts drawn #461; tools to add a rail and a duct, size a duct, drop a device onto a rail (none yet) |
| wire-routing | Routes overlay #460 plus its toggle and hook; ducts selectable |
| cut-lengths | Reports hub #452 with the Cut list; an in-app way to add the cut list page |
| enclosure-door-and-depth | #409 |
| heat-and-sizing | Heat layer #406; sizing UI (`cabinet.size-enclosure`, engine on main) |
| panel-plate-drawing | `export.plate` #470, door sheet size #477 |
| panel-labels | Labels report #482, Reports hub #452 (`view.reports`) |
| view-3d | #481, #457, #391 and the 3D face with `cabinet.view3d` |

## Asked of other threads
- Anchors (owner: Cabinet UI thread, framework registers): `switcher.cabinet`, `switcher.reports`, `outline.pages`,
  `palette.field`, `cabinet.canvas`, `cabinet.unplaced`, `cabinet.place_all`, `cabinet.accept`, `cabinet.plate`,
  `cabinet.table`, `cabinet.problems`, `cabinet.inspector`, `cabinet.heat`, `cabinet.routes`, `cabinet.door`,
  `cabinet.door.apply`, `cabinet.add_rail`, `cabinet.add_duct`, `cabinet.sizing`, `cabinet.sizing.recommend`,
  `cabinet.view3d`, `cabinet.view3d.cube`, `reports.labels`, `reports.table`, `reports.layout`, `reports.export`,
  `reports.cutlist`.
- Signals (raised by the Cabinet): `cabinet_dropped` (a device dropped from the list or moved), `cabinet_plan_accepted`,
  `cabinet_rail_added`, `cabinet_duct_added`.
- Framework thread (cse_018gxMVUe63W8rvrNBwroP8s): state checks `placed`, `rail` and `duct` (and a count kind
  for placements) so ticks do not depend on signals; `solve` ops for the cabinet (place a device, Place all);
  a canvas target for plate coordinates (`at` and `area` name schematic pages only).

## Traps
- A lesson's `project` may name a sibling lesson's file (`../wire-routing/project.json`); keep those files in place.
- Step text stays under 500 characters (lint). `pause = true` is only for steps that also have an `expect`.
- Depth numbers in enclosure-door-and-depth come from #409's defaults (usable depth 230 mm, 80 mm at depth 100).

## State after the second rewrite batch (2026-10-16, main aa5977ab)
All ten cabinet lessons now have what/why text, Dutch for every step and stand-alone wording. All stay `draft` or `early` per the lesson plan; none has `how` yet (waiting for the framework field).
| Lesson | Verified against main | Still missing in the app |
| --- | --- | --- |
| cabinet-layout, rails-and-ducts | yes: Place all (rules planner, 5 rails, 5 ducts), Accept, Plate card, Rules, Cut list | anchors, signals cabinet_dropped, cabinet_rotated, cabinet_removed, cabinet_plan_accepted; `command` check cannot see edit.rotate and edit.delete |
| wire-routing | yes, numbers from the overlay code on `wire-routing/project.json` | anchors; Place all joins none of its ducts (every wire free air) |
| cut-lengths | yes: wire list basis, Export wire cut | signal wire_cut_exported |
| enclosure-door-and-depth | yes: Inside and door section of the Plate card | anchors; Plate card only shows with nothing selected |
| heat-and-sizing | yes: Heat chips, Simulate, Size enclosure with an added enclosure | anchors; legend has no loss totals; a part without loss counts as 0 W silently |
| panel-plate-drawing | yes: export.plate, 20 + 4 holes, 2 cut-outs | the header button does not raise export.plate |
| panel-labels | command line only | no Labels page in the Reports workspace; CLI reads .novo, not JSON |
| view-3d | NOT verifiable | no 3D face, no cabinet.view3d command, no view cube |
| commissioning | yes: tabs Checks and Punch list, Pass, Fail, N/A, punch items | Fail does not open a punch item (lesson says so); signal punch_added and anchors |
The six lessons after wire-routing start from `../wire-routing/project.json`.

## State after #624, #643 and #654 (2026-10-16, main 7ccc024c and later)

- All ten cabinet lessons are `early`, with Dutch and `how`/`how_nl`. Samples come from Place all (trunk duct included): `cabinet-layout/finished.json` equals `wire-routing/project.json`; regenerate with `cargo run -p novo-app --example panel_lessons assets/lessons`.
- Numbers on the sample: routes 490/579/643/1047 mm, 11 wires at fullest duct 10 %, wire list 19 rows (15 routed, 4 estimate), Cut list summary 4 wires 0.88 m and 15 wires 9.32 m, Labels 38/4/9/1, plate holes 20 + 4.
- Still open: anchor `workspace.view3d` (3D steps), solve ops for cabinet actions and `solve = "manual"` for wire_cut_exported (runner thread), signals cabinet_dropped, cabinet_rotated, cabinet_removed, cabinet_plan_accepted, wire_cut_exported. Promote lessons to `ready` once those exist and the replay gate passes.
