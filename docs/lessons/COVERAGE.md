# Lesson coverage: what a KoperCAD user must learn, and where it is taught

Status: first audit, 2026-10-22, main `61e00a3` (37 lessons, 4 paths, 6 tracks).
Owner: Learning (this table is kept current by whoever adds or changes a lesson; the rule is at the end).

How this was made: the concepts come from the product itself (the 296 commands in
`novo-workflow/src/commands.rs`, the eight menus, `docs/ROADMAP.md`, `docs/BUILD-MODE.md`,
`docs/PROTECTION.md`, `docs/functional-safety.md`), from the real-machine build
(`audit/real-machine/findings.md`) and from the learning auditors (`audit/learning/critic/round1-defects.md`,
`audit/learning/creative/`). Each concept was matched to the steps of every `lesson.toml`. Nothing was
run for this table: it says what the lesson texts teach, not that the lessons still tick (the replay gate
`every_shipped_lesson_replays_from_its_own_solve` does that).

Depth: **full** = a learner does it and a check ticks; **partial** = mentioned or one corner of it;
**none** = no lesson.

The example column says whether `Open example` (the motor starter in `app/example.rs`) shows the concept
at all, so a learner can try it on a real drawing. That was read from `example.rs`, not run.

## What changed since the auditors wrote

LC-03 and LC-04 are closed: every one of the 37 lessons is in a path, and a path never teaches a lesson
before something it `needs` (test `paths_teach_prerequisites_first_and_cover_every_lesson`). The four
"fix the circuit" challenges and "Predict the starter" exist. Still open from the critic: LC-15 (the Dutch
text is never shown), LC-19 (Excel steps cannot tick), LC-29 (gesture-only steps).

## 1. Concept map

| # | Concept | Lesson | Depth | In the example | Gap and plan |
| --- | --- | --- | --- | --- | --- |
| 1 | Fast keys, command field, wheel, place a part | `placing-parts` | full | no | The command field (Ctrl+K) as the way to find anything is taught only in passing |
| 2 | Wiring modes, pin to pin | `wiring-modes` | full | no | Wire end-on-part, next pin, tidy routes: none (`wiring-craft`) |
| 3 | Potentials and feeds | `potentials` | full | yes | LC-21 drag step needs a hand check on Windows |
| 4 | Tags, locations, pages and the outline (`=plant +location -device`) | motor-starter step 1 only | partial | yes | `tags-and-pages` |
| 5 | Edit moves: copy, paste (keep tags), mirror, notes, text | placing-parts (copy, undo, rotate) | partial | no | `wiring-craft` |
| 6 | A motor starter, control and power circuit | `motor-starter` | full | yes | |
| 7 | Cross references, jump to partner, every occurrence | `cross-references` | full | yes | Net trace and "Pages and devices on this net" none |
| 8 | Signals across pages (interruption points) | `interruption-points` | full | no | |
| 9 | Macros: save, place, share | `blocks-and-macros` | full | no | Starter macros pack, parameters and export pack none |
| 10 | Autopilot and auto-wiring (complete the wires) | none | none | yes | `autopilot-wires` |
| 11 | Terminal strips: proposals, jumpers, PE | `terminal-strips` | partial | yes | Rename, renumber, spares, join, kind, level, order, style: `terminals-in-depth` |
| 12 | Terminal connection screens | `connection-screens` | full | yes | |
| 13 | Cables, cores, shields | `cables-and-shields` | full | yes | Retype, rename, restore declined: `terminals-in-depth` |
| 14 | Wire numbers and potentials numbering | `wire-numbers` | partial | yes | Written before the Numbers window and the numbering schemes; check, conflicts, locks: `wire-numbering-in-depth` (also re-verify the lesson) |
| 15 | PLC cards, channels, addresses | `plc-io` | partial | no | I/O list import, tag import, TIA / Studio 5000 / PLCopen export, address plan: `plc-lists` |
| 16 | Design checks, waive | `design-checks` | full | yes | |
| 17 | Calculations, protection curves, short circuit, single-line diagram | motor-starter mentions it | none | yes | `protection-and-single-line` |
| 18 | Safety functions, PL / PFHd, safety sheets | none | none | yes | `safety-functions` |
| 19 | Standards check and compliance report | none | none | yes | `standards-check` |
| 20 | Live view: operate, coil, contacts | `live-view` plus 4 challenge lessons | full | yes | |
| 21 | Why is this lit or dead; fault scenarios; motor start; PLC program simulation | none | none | yes | `why-and-faults` |
| 22 | Cabinet layout, place all, rules | `cabinet-layout`, `rails-and-ducts` | full | yes | Enclosure catalog and cabinet surfaces: partial |
| 23 | Door and depth, enclosure sizing | `enclosure-door-and-depth`, `heat-and-sizing` | full | yes | |
| 24 | Wire routing, ducts, cut lengths | `wire-routing`, `cut-lengths` | full | yes | |
| 25 | Plate drawing, drill table, labels | `panel-plate-drawing`, `panel-labels` | full | yes | |
| 26 | Heat: preview, simulation, cooling verdict | `heat-and-sizing` | full | yes | 3D heat, airflow, contours, probe, compare (heat is only taught on the 2D plate): `heat-in-3d` |
| 27 | 3D view: look around | `view-3d` | full | yes | |
| 28 | 3D editing (move, align, measure, duplicate, place), section plane, saved views, cable and wire runs, duct fill | none | none | yes | `editing-in-3d` |
| 29 | Workshop: bench output, bench packet, ferrule table, assembly checklist, workshop pack | none (cut-lengths, labels, plate drawing are one piece each) | partial | yes | `workshop-pack` |
| 30 | Build mode at the plate | none | none | yes | `build-mode` |
| 31 | Commissioning: checks, punch list, report | `commissioning` | full | yes | Photos, redlines, signature, field results: partial |
| 32 | Generated pages, wire list, bill of materials | `generated-pages`, `wire-list-and-bom` | full | yes | Report designer, cable schedule, plug list, single-line page: partial |
| 33 | PDF export, bookmarks, links, issue freeze | `pdf-export` | full | yes | Print preview, plot sets, scale check, change report, per-variant PDF: `handover-package` |
| 34 | Revisions, compare, page lock and restore | `revisions-and-compare` | full | no | Lock page, restore page: partial |
| 35 | Handover package (open-problems gate, folder) | none | none | yes | `handover-package` |
| 36 | Parts library, one-click import | `parts-library` | full | yes | Folder, table and CAD parts-export import, STEP model, auto-assign, remove unused: `parts-in-bulk` |
| 37 | Spreadsheets round trip | `spreadsheets` | full | no | LC-19: the Excel steps cannot tick |
| 38 | DXF out and in, background | `dxf-and-backgrounds` | full | no | DWG converter: partial |
| 39 | Import a whole project (EPLAN PXF, AutomationML, crane configurator) | none | none | no | `bring-a-project-in` |
| 40 | Variants, machine options, generate pages from a spec sheet | none | none | no | `variants-and-options` |
| 41 | Drawing standard IEC / NFPA | `drawing-standard` | full | no | |
| 42 | Preferences, keymap file, project settings | `time-savers` | full | no | |
| 43 | Data safety: save, autosave, recover, read-only open, `.kopercad` file, older / newer files | none | none | yes | `your-project-file` |
| 44 | Workspaces, panels, focus mode, reset layout, full screen | none | none | yes | `find-your-way` |
| 45 | Ask Claude (assistant) | none | none | no | `ask-claude` (needs an API key to run; the lesson reads, the replay checks the window opens) |
| 46 | Help: learn, what's new, About the numbers, report a problem | placing-parts mentions Learn | partial | no | folded into `find-your-way` |
| 47 | Language (Dutch) | none | none | no | Blocked on LC-15 (lesson language setting) |

Count: 47 concepts. Full: 26. Partial: 7. None: 14. (After the plan below every concept has a lesson except
47, which waits on the language setting.)

## 2. The example project

`Open example` builds the Gate 1 motor starter on two pages with: wire numbers, the generated pages
(contents, terminal diagram, cable diagram, lists), a terminal strip `-X1`, a cable, a cabinet `+CAB` with
rails, ducts, a right-hinged door and real part data (so heat and 3D have numbers), a stated 400 V / 6 kA
supply, and one safety function.

It therefore carries concepts 3, 4, 6, 7, 10 to 14, 16 to 21 (live view), 22 to 36, 43 and 44. Lessons for
those can run on `project = "@example"` and need no `project.json` of their own, which also keeps them
from drifting from the program.

It does **not** show, and should not pretend to: PLC cards and an I/O list (15), signals across pages (8),
macros (9), revisions (34), variants and options (40), imports (39), the other drawing standard (41).
Those lessons keep their own sample project. Extending the example is not needed to teach them; adding a
PLC card, a revision and a macro to it would make the first-run example less clean and the CV200 machine
that would show them honestly is blocked (real-machine verdict: RM-03, RM-04, RM-11 and RM-17 open).
Decision: the example stays a single motor starter. Revisit when those four close.

## 3. The plan: new lessons

Each is 6 to 10 minutes, English and Dutch text, a `solve` for every action step, replayed in CI and
walked in the QA harness at 1366x768 and 1920x1080. Order is by how much a missing lesson costs a user.

| Batch | Lessons | Track | Path |
| --- | --- | --- | --- |
| A, data and trust | `your-project-file`, `protection-and-single-line`, `safety-functions`, `standards-check` | start, outputs | Project lead; First hour gets `your-project-file` |
| B, the shop floor | `workshop-pack`, `build-mode`, `handover-package`, `autopilot-wires` | cabinet, outputs, circuits | Panel builder, Project lead, Draftsperson |
| C, craft | `tags-and-pages`, `wiring-craft`, `terminals-in-depth`, `wire-numbering-in-depth` | start, circuits, terminals | Draftsperson |
| D, depth | `why-and-faults`, `editing-in-3d`, `heat-in-3d`, `plc-lists` | circuits, cabinet, terminals | Panel builder |
| E, scale | `variants-and-options`, `bring-a-project-in`, `parts-in-bulk`, `find-your-way`, `ask-claude` | data, start | Project lead |

Rules for the batches: one pull request per batch; paths and `needs` stay valid (the lint fails
otherwise); a lesson names the real menu path and the command field phrase of what it teaches; an
estimate is called an estimate on the lesson card the same way the program calls it.

Lessons that already exist keep drifting while the program moves (wire numbering gained a Numbers window,
terminal strips gained kinds). Re-verifying them needs a human look at each card, and is listed as
batch C work for `wire-numbers` and `terminal-strips`; the replay gate only proves a lesson still ticks,
not that its words still describe the screen.

## 4. The rule that keeps this table true

A pull request that adds a user-facing feature names, in its description, the lesson it adds or changes,
or says why none is needed. A pull request that adds or removes a lesson updates the table above.
