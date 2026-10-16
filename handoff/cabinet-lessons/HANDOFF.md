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
