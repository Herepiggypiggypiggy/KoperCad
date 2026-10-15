# Roadmap summary

Full roadmap with dependencies and done-criteria:
https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e

State as of 2026-10-15, main `d91b4c2`. "Implemented" means merged on main; builds 1 to 4 are published
(build 4 is main `f6be257`), and nothing after that is in a build yet. Open pull requests are listed
under [In review](#in-review-right-now).

| Phase | Workstreams | Gate at the end | State |
| --- | --- | --- | --- |
| 0 Foundation | 01 App shell and CI, 02 Data model, 03 Canvas engine, 04 Project file | Model API v0 frozen, canvas draws it | Done |
| 1 Schematics | 05 Symbol library, 06 Schematic editor, 07 Parts database, 08 Fast workflow, 09 PDF and print | Motor starter drawn over 2 pages, saved, printed to PDF | Done |
| 2 Intelligence | 10 Cross references, 11 Terminal strips, 12 Cables, 13 Reports, 14 Design checks, 28 Python API and CLI | Terminal, cable and report pages generate live | Met on Linux |
| 3 Physical design | 15 Cabinet layout, 16 Wire routing, 17 PLC and I/O, 18 Revisions | | PLC, revisions and the cabinet layout engine with placement storage are on main; wire routing has its design and a pure duct router; Changes mode and the Issue revision window are in the app |
| 4 Simulation (after Gate 1) | 19 Sim engine, 20 Device models, 21 Animation, 22 Fault scenarios | Star-delta starter simulates and animates | Live potentials, coil, changeover and timer contacts, lamp and motor glow are on main (awaiting hands-on test); device models and animation (#346) in review |
| 5 AI and interop | 23 AI assistant, 24 Import/export, 25 Standards packs, 26 3D cabinet, 27 Release polish | | Started: DXF reader and writer (#344), symbol packs design (#335); EPLAN parts import (#347) and the NFPA pack (#351) in review |

## Near term: the 26-point list (owner, 2026-10-15 12:34 UTC)

The first hands-on tests of the experimental builds produced a list of 26 workflow points. It is the
near-term roadmap and is built in parallel across threads; the design is in `docs/DESIGN.md` (#238,
#242, #247). Every point is now merged on main; 14, 16 and the simulation UI of point 22 await the owner's hands-on test.

| # | Point | State | PRs on main / notes | Owner |
| --- | --- | --- | --- | --- |
| 1 | Auto connections drawn by default, setting for ask or never | Implemented | #250, Preferences Draw, Ask, Never (#261) | Schematic editor, App shell |
| 2 | Double-click on the grid starts a wire in any tool | Implemented | #226, #266 | Canvas engine |
| 3 | Wire snaps to the grid only, no smooth snap | Implemented | #262 | Schematic editor |
| 4 | Move a component's text on its own | Implemented | #254, #265 | Canvas engine |
| 5 | Dragging a component moves only the component | Implemented | #259; Shift+M and Shift+arrows carry wires (#294, #324 in review) | Schematic editor |
| 6 | Wire corners always visible and selectable, a click never selects a whole wire | Implemented | #246, #260, #267, #269, #274, #288 | Canvas engine |
| 7 | Holding Q opens the wheel only, no `qqqq` in the search | Implemented | #244 | App shell |
| 8 | Wheel has Other in the middle, Tab 5 holds interruption, feed and ground | Implemented | #268 | Workflow, Layout |
| 9 | R rotates before and after placing; feeds number L1, L2, L3 | Implemented | #293 | Fast keyboard |
| 10 | Interruption points that cross reference between pages and auto-number | Implemented | #240, #272 | Potentials |
| 11 | Toggle PE terminal by right-click, automatic PE marking | Implemented | #239, #253, #266, #270, #272, #325 | Potentials, App shell |
| 12 | Pages navigator with groups and panels | Implemented | #241, #248, #257, #303, #314; group and panel rename in review (#348) | Pages |
| 13 | Feed labels and terminal numbers on the same side | Implemented | #242 | Layout |
| 14 | Wheel does not jump to a wedge the pointer only crosses | Implemented, awaiting hands-on test | Dwell before the outer ring follows (#244) | App shell |
| 15 | Dragging a one-pole part gives a 2 to 5 pole part on terminal spacing | Implemented | #249, #230 | Terminals and strips |
| 16 | Picking in the first ring does not place until the second ring is chosen | Implemented, awaiting hands-on test | Held place key and dwell (#244) | App shell |
| 17 | Auto connections also made when passing through another object | Implemented | #250, #296 | Schematic editor |
| 18 | Longer part search and Ctrl+K menus | Implemented | #266; palette ranking in review (#350) | App shell |
| 19 | Corner mode places oriented corner and T pieces, as in EPLAN | Implemented | #246, #263, #283 | Schematic editor |
| 20 | `C` places a cable along a line over wires | Implemented | #243, cable tool (#318) | Fast keyboard |
| 21 | Q terminates a wire in the object, Tab cycles connection points | Implemented | #306, #327 | Fast keyboard |
| 22 | Wires inherit potential from terminals and feeds, live under simulation | Implemented, simulation UI awaiting hands-on test | #255, #271, #276 to #281, #297, #310, #326; PLC outputs and inputs in review (#333) | Potentials |
| 23 | Right-click a feed to set its potential | Implemented | #245, #270, #275 | Potentials, Workflow, Layout |
| 24 | Select terminals, make them connected terminals | Implemented | #249, #251, #299 | Terminals and strips |
| 25 | Standard terminal from the picker is one-ended | Implemented | #249 | Terminals and strips |
| 26 | New strips take the next free -X number, adjacent ones inherit; Up and Ctrl+Up | Implemented | #249 | Terminals and strips |

Points 14 and 16 and the whole live simulation view (Shift+L, coils, contacts, timers, glow) need the
owner's hands-on test on Windows before they count as done. See `docs/WINDOWS-CHECK.md`.

## New since the 26-point list (PR #258 onwards)

- **Potentials and live simulation:** live view with coil to contact coupling, changeover and timer
  contacts with seconds left, hover on a wire, lit wires under symbols, theme tokens, lamps and motors
  glowing (#271, #276 to #281, #297, #310, #326); inspector Time field for coils and timers (#300, #319).
  PLC outputs and inputs in review (#333); device models and animation (#346) in review.
- **Cable tool:** `C` draws a line and the wires it touches become one cable (#318).
- **Cabinet layout:** design (#284), `novo-layout` geometry engine (#287), rails and overflow query
  (#289), place-all planner (#320), in-memory layout state (#322), placement storage in the model
  (#340), cabinet scene on the canvas (#349).
- **Wire routing through ducts:** design and pure duct routing engine (#334).
- **Interop:** design and DXF reader and writer (#344); EPLAN parts import in review (#347).
- **Revisions:** Issue revision window and command (#290), Changes mode and list (#305, #323),
  title block revision fields (#298, #329); revision table above the title block is a draft (#343).
- **Symbol packs:** design (#335); NFPA core set in review (#351).
- **Parts library plan:** design for Wago, Phoenix Contact, Schneider and Siemens catalogues with a
  one-click Import design, in review (#342).
- **Keymap file:** user `keymap.txt` (#341) and Ctrl+D smart duplicate (#331).
- **Performance:** 10,000-symbol timings and neighbour grid for strip runs (#328), far wires skipped by
  bounds (#316, #315), live view repaints only on change (#273).
- **Rename:** `kopercad.exe`, Python module and CLI, data folder migration (#304, #313, #302).

## In review right now

#324 Shift+arrows nudge, #333 live potentials for PLC, #339 open-end ring radius, #342 parts library
design, #343 revision table (draft), #346 device models and animation, #347 EPLAN parts import,
#348 rename group or panel, #350 palette ranking, #351 NFPA pack.

## Next after this

- Wire routing in the app (ducts, wire lengths) on the finished engine; cabinet layout dialogs.
- PLC dialogs on the finished engine; parts library import (#342) once agreed.
- Simulation: fixpoint for latching circuits, fault scenarios, then the AI assistant (WS-23).
- Windows hands-on verification of everything after build 4, then installer polish (per-user, no admin rights).
- DXF export dialogs, Excel round trip, more symbol packs.
