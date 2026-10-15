# Roadmap summary

Full roadmap with dependencies and done-criteria:
https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e

State as of 2026-10-15, main `05253e1`. "Implemented" means merged on main; builds 1 to 4 are published
(build 4 is main `f6be257`), and nothing after that is in a build yet.

| Phase | Workstreams | Gate at the end | State |
| --- | --- | --- | --- |
| 0 Foundation | 01 App shell and CI, 02 Data model, 03 Canvas engine, 04 Project file | Model API v0 frozen, canvas draws it | Done |
| 1 Schematics | 05 Symbol library, 06 Schematic editor, 07 Parts database, 08 Fast workflow, 09 PDF and print | Motor starter drawn over 2 pages, saved, printed to PDF | Done |
| 2 Intelligence | 10 Cross references, 11 Terminal strips, 12 Cables, 13 Reports, 14 Design checks, 28 Python API and CLI | Terminal, cable and report pages generate live | Met on Linux |
| 3 Physical design | 15 Cabinet layout, 16 Wire routing, 17 PLC and I/O, 18 Revisions | | PLC and I/O and revisions engines done; layout and routing not started |
| 4 Simulation (after Gate 1) | 19 Sim engine, 20 Device models, 21 Animation, 22 Fault scenarios | Star-delta starter simulates and animates | Started: live-potentials solver (#255) |
| 5 AI and interop | 23 AI assistant, 24 Import/export, 25 Standards packs, 26 3D cabinet, 27 Release polish | | Not started |

## Near term: the 26-point list (owner, 2026-10-15 12:34 UTC)

The first hands-on tests of the experimental builds produced a list of 26 workflow points. It is the
near-term roadmap and is built in parallel across threads; the design is in `docs/DESIGN.md` (#238,
#242, #247). Each point below is one of: **Implemented** (merged on main), **Partly** (the engine or
model is merged, the UI or behaviour is not), **In progress** (a thread is building it, nothing merged
yet), **In review**, or **Designed** (specified in DESIGN.md, not built).

| # | Point | State | On main / notes | Owner |
| --- | --- | --- | --- | --- |
| 1 | Auto connections drawn by default, setting for ask or never | Partly | Drawn with the drop in one undo step (#250). The Preferences setting (Draw, Ask, Never) is not on main | Schematic editor, App shell |
| 2 | Double-click on the grid starts a wire in any tool | In progress | Double-click with the Connect tool ends or starts a wire (#226) | Canvas engine |
| 3 | Wire snaps to the grid only, no smooth snap | In progress | | Schematic editor |
| 4 | Move a component's text on its own | In progress | Labels now win picks over symbol ink (#254) | Canvas engine |
| 5 | Dragging a component moves only the component | In progress | Decided: plain drag moves the component only, Shift+drag takes its wires | Schematic editor |
| 6 | Wire corners always visible and selectable, a click never selects a whole wire | Partly | Corner handles and wire parts in the canvas model (#246); click, drag and double-click behaviour not on main | Canvas engine |
| 7 | Holding Q opens the wheel only, no `qqqq` in the search | In progress | Wheel dwell and held place key (#244) | App shell |
| 8 | Wheel has Other in the middle, Tab 5 holds interruption, feed and ground | Designed | The wheel still has a fifth `More...` wedge | Workflow, Layout |
| 9 | R rotates before and after placing; feeds number L1, L2, L3 | Partly | R while placing existed; feed naming helper exists; rotate on a selection and auto-numbering in the app are open | Fast keyboard |
| 10 | Interruption points that cross reference between pages and auto-number | Partly | Auto-naming engine (#240); xref labels exist; app hook-up open | Potentials |
| 11 | Toggle PE terminal by right-click, automatic PE marking | Partly | Toggle and automatic marking (#239), manual override flag (#253, #245); right-click menu not built | Potentials, App shell |
| 12 | Pages navigator with groups and panels | Implemented | Model (#241) and navigator in the outline (#248). Removal of the Recent list is in review (#257) | Pages |
| 13 | Feed labels and terminal numbers on the same side | Implemented | Tags, terminal numbers and feed names sit left of the symbol (#242) | Layout |
| 14 | Wheel does not jump to a wedge the pointer only crosses | Implemented | Dwell before the outer ring follows (#244); confirm hands-on | App shell |
| 15 | Dragging a one-pole part gives a 2 to 5 pole part on terminal spacing | Implemented | Multi-pole drag (#249) | Terminals and strips |
| 16 | Picking in the first ring does not place until the second ring is chosen | Implemented | Held place key and dwell (#244); confirm hands-on | App shell |
| 17 | Auto connections also made when passing through another object | In progress | | Schematic editor |
| 18 | Longer part search and Ctrl+K menus | In progress | | App shell |
| 19 | Corner mode places oriented corner and T pieces, as in EPLAN | Partly | Corner handles drawn from stored routes (#246); placing oriented pieces not on main | Schematic editor |
| 20 | `C` places a cable along a line over wires | Partly | Key and commands (#243); cable drawing in the shell is open | Fast keyboard |
| 21 | Q terminates a wire in the object, Tab cycles connection points | In progress | Tab holds a proposal today | Fast keyboard |
| 22 | Wires inherit potential from terminals and feeds, live under simulation | Partly | Declared potentials stored; live solver `novo_sim::solve` (#255). Coil to contact coupling is held back | Potentials |
| 23 | Right-click a feed to set its potential | Partly | Set potential engine (#245); chips and menu not built | Potentials, Workflow, Layout |
| 24 | Select terminals, make them connected terminals | Implemented | Connect and disconnect (#249). An older overlapping PR (#251) is being trimmed | Terminals and strips |
| 25 | Standard terminal from the picker is one-ended | Implemented | #249 | Terminals and strips |
| 26 | New strips take the next free -X number, adjacent ones inherit; Up and Ctrl+Up | Implemented | #249 | Terminals and strips |

Also open in review: double-click rename of terminals, feeds and interruption points (#252; #237
already renamed tags and wire numbers), which the owner reported as not working in build 4.

## Next after the list

- Revisions in the app (issue dialog, Changes mode, title block fields) and PLC dialogs on top of the
  finished engines.
- Cabinet layout by panel (Phase 3), then wire routing through ducts.
- Simulation: coil to contact coupling, fixpoint for latching circuits, timers, live tint on the canvas.
- Windows verification of everything after build 4, then installer polish (per-user, no admin rights).
- The rest of the rename (executable, Python module, data folders with a migration).
