# The lesson plan: tracks, order, levels and how the menus teach

Status: plan, first part built (tracks, order, `needs` and the `early` status are data in `assets/lessons`).
Owner: the lesson-plan thread, with the Learning modules thread (engine) and the lesson content threads.
Builds on `docs/design/learning-modules.md` (engine and format) and `docs/design/discoverability.md`
(shortcut sheet, tips, Help menu; DISC-1 and DISC-2 own those).

## Why this page

The program has 33 lessons and a lesson engine, but the lessons were all `draft`, so Build 6 showed an
empty Learn window, and the lessons themselves taught too little: a step said *what* to do, not *how*,
and "Show me how" only flickered an area. A person who opens the program on day one needs a path, not a
list: where to start, what comes next, how long it takes, and a menu that always knows where they are.

## 1. Curriculum

Six tracks, in the order a person meets the work. Inside a track the order is the order to do them.
`needs` is advice shown as "Do first", never a lock. Total: 33 lessons, about 4 hours 15 minutes.

### Start here (5 lessons, about 32 min)

Place parts with the keyboard, wire them, give them a supply and set the program up for you.

| # | Lesson | Nederlands | Level | Min | Do first | Topics |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `placing-parts` Placing parts fast | **missing** | beginner | 6 | - | placing, fast keys |
| 2 | `wiring-modes` Corners and Lines | **missing** | beginner | 7 | placing-parts | wiring |
| 3 | `potentials` Potentials and feeds | **missing** | beginner | 6 | wiring-modes | potentials, feeds |
| 4 | `time-savers` Three settings that save time every day | **missing** | beginner | 7 | - | settings, preferences, keymap, standard |
| 5 | `drawing-standard` IEC or NFPA: the Drawing Standard | IEC of NFPA: de tekennorm | beginner | 6 | - | symbols, standard, project settings |

### Circuits (4 lessons, about 34 min)

Draw a motor starter, follow a coil to its contacts, try it live and reuse it as a macro.

| # | Lesson | Nederlands | Level | Min | Do first | Topics |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `motor-starter` A motor starter | **missing** | intermediate | 12 | potentials | motor, control circuit, tags |
| 2 | `cross-references` Cross references | **missing** | intermediate | 8 | motor-starter | cross references, contacts |
| 3 | `live-view` The live view | **missing** | intermediate | 6 | cross-references | simulation, live |
| 4 | `blocks-and-macros` Save a circuit once, place it anywhere | **missing** | intermediate | 8 | motor-starter | macro, block, library, share |

### Terminals, cables and PLC (6 lessons, about 49 min)

Terminal strips and their plans, signals across pages, cables and shields, wire numbers and PLC cards.

| # | Lesson | Nederlands | Level | Min | Do first | Topics |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `terminal-strips` Terminal strips | Klemmenstroken | beginner | 8 | - | terminals |
| 2 | `connection-screens` Terminal connection screens | Klemmenplannen | beginner | 8 | terminal-strips | terminals, generated-pages |
| 3 | `interruption-points` Signals across pages | Signalen over pagina's | intermediate | 9 | connection-screens | cross-references, interruption-points |
| 4 | `cables-and-shields` Cables and Shields | Kabels en schermen | intermediate | 8 | terminal-strips | cables, terminals, shield |
| 5 | `wire-numbers` Wire Numbers and the Wire List | Draadnummers en de draadlijst | intermediate | 7 | cables-and-shields | wire numbers, wire list, potentials |
| 6 | `plc-io` PLC Cards and I/O | PLC-kaarten en I/O | intermediate | 9 | - | plc, io, simulation |

### Checks, lists and issuing (5 lessons, about 41 min)

Design checks, the pages KoperCAD writes for you, wire list and bill of materials, revisions and PDF.

| # | Lesson | Nederlands | Level | Min | Do first | Topics |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `design-checks` Find and fix problems before anyone else does | **missing** | beginner | 8 | - | checks, problems, waive |
| 2 | `generated-pages` Pages KoperCAD Writes For You | Pagina's die KoperCAD voor je schrijft | beginner | 8 | terminal-strips, cables-and-shields | reports, pages, outline |
| 3 | `wire-list-and-bom` Wire List and Bill of Materials | Draadlijst en stuklijst | beginner | 9 | generated-pages, cables-and-shields | reports, parts, export |
| 4 | `revisions-and-compare` Issue a revision and see what changed | **missing** | beginner | 8 | - | revision, compare, changes |
| 5 | `pdf-export` Issue and Export a PDF | Uitgeven en exporteren als PDF | beginner | 8 | wire-list-and-bom | export, pdf, revisions |

### Cabinet and panel building (10 lessons, about 74 min)

Lay out the plate, size and route, check door and heat, then drawings and labels for the workshop, 3D and commissioning.

| # | Lesson | Nederlands | Level | Min | Do first | Topics |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `cabinet-layout` Lay out the mounting plate | De montageplaat indelen | beginner | 8 | - | cabinet, layout |
| 2 | `rails-and-ducts` Rails and wiring ducts | Rails en bedradingskanalen | beginner | 8 | cabinet-layout | cabinet, rails, ducts |
| 3 | `wire-routing` Route the wires and watch the ducts | Draden routeren en de kanalen in de gaten houden | intermediate | 8 | rails-and-ducts | cabinet, routing, ducts |
| 4 | `cut-lengths` Wire list and cut lengths | Draadlijst en afkortlengtes | intermediate | 8 | wire-routing | cabinet, wire-list, routing |
| 5 | `enclosure-door-and-depth` Will the door close? | Sluit de deur nog? | intermediate | 7 | cabinet-layout | cabinet, enclosure, door |
| 6 | `heat-and-sizing` Where it gets hot, and how big the cabinet must be | Waar het warm wordt en hoe groot de kast moet zijn | intermediate | 8 | cabinet-layout | cabinet, heat, sizing |
| 7 | `panel-plate-drawing` The plate drawing and drill table | De plaattekening en boortabel | intermediate | 8 | cabinet-layout | cabinet, plate-drawing, export |
| 8 | `panel-labels` Labels for wires, terminals and devices | Labels voor draden, klemmen en componenten | beginner | 6 | cut-lengths | cabinet, labels, export |
| 9 | `view-3d` The cabinet in 3D | De kast in 3D | beginner | 5 | cabinet-layout | cabinet, 3d |
| 10 | `commissioning` Commission a panel with a tick list | **missing** | intermediate | 8 | - | commissioning, punch list, test |

### Parts and data (3 lessons, about 23 min)

Find and import parts, edit hundreds of devices in Excel and hand over DXF.

| # | Lesson | Nederlands | Level | Min | Do first | Topics |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `parts-library` Find a part and import it in one click | **missing** | beginner | 7 | - | parts, library, import |
| 2 | `spreadsheets` Edit two hundred devices in Excel | **missing** | intermediate | 9 | parts-library | spreadsheet, excel, export, import |
| 3 | `dxf-and-backgrounds` Hand over DXF and trace a floor plan | **missing** | intermediate | 7 | - | dxf, export, import, background |

### Learning paths (routes across the tracks)

A path is a reading order for a role, shown in the Learn window above the tracks. Data: `paths.toml`
(`id`, `title`, `title_nl`, `summary`, `lessons = [...]`); a path is only a list, progress comes from the lessons.

| Path | For | Lessons |
| --- | --- | --- |
| First hour | Anyone new | placing-parts, wiring-modes, potentials, motor-starter, cross-references |
| Draftsperson | Draws the schematics | First hour, then interruption-points, terminal-strips, cables-and-shields, wire-numbers, design-checks |
| Panel builder | Builds the cabinet | cabinet-layout, rails-and-ducts, wire-routing, cut-lengths, enclosure-door-and-depth, panel-plate-drawing, panel-labels, commissioning |
| Project lead | Issues and hands over | generated-pages, wire-list-and-bom, revisions-and-compare, pdf-export, spreadsheets |

### Levels and time

- **Beginner**: no earlier lesson needed beyond `needs`; one idea; 5 to 8 minutes.
- **Intermediate**: assumes the First hour path or the named lessons; two to three ideas; 6 to 12 minutes.
- **Advanced**: reserved for lessons that combine tracks (none yet; macros with variants, bulk edit by sheet).
- A lesson longer than 12 minutes is split. `minutes` is what a first-time reader needs, measured by
  replaying the lesson's `solve` steps and adding reading time.

### Language

Every lesson, track and path has an English title and a `title_nl`. Dutch is complete for the tracks and
for 19 of 33 lessons today; the lint lists the rest (`missing_nl`), and a lesson cannot move from `early`
to `ready` without its Dutch titles and step texts.

## 2. Which lessons are visible: `ready`, `early`, `draft`

| Status | Learn window | Badge | Meaning |
| --- | --- | --- | --- |
| `ready` | Shown | none | Every step has a `demo`, a working check, a demo; replayed headless in CI |
| `early` | Shown by default | "Early access" | Runs, but steps may be rough or name controls that are unfinished |
| `draft` | Hidden (Preferences, "Show draft lessons") | "Draft" | May name commands or anchors this build does not have |

All 33 lessons move from `draft` to `early` now, so the next build has a full Learn window. A lesson is
promoted to `ready` when it meets the teaching standard below; the lint says what is missing.

## 3. The teaching standard (what makes a step teach)

Tadjiro's test of "Placing parts fast": the card said "Hold Q for the wheel" but gave no sense of how, and
"Show me how" flickered an area. A step is complete only if it has all of:

1. **What and why**: one sentence on the goal of the step and why a person would do it.
2. **How** (`demo = [...]`, PR #601; the card lists these as the "How" lines): the concrete keys and clicks (`press`, `type`, `hold`,
   `move`, `drag`, `ghost`, `say`), keys written as `{key:command.id}` so a rebound key reads right.
3. **Show me** (a demo, not a highlight): the pointer moves along the real path, keycaps appear as the
   demo presses them, and the result shows as a ghost on the canvas. Driven by the step's `demo` list (PR #601), or derived from `expect`, `solve` and `point`; it never changes the drawing.
4. **Try it**: after the demo the step waits for the person's own action and ticks on `expect`.
   The demo can be replayed; "Do it for me" (applies `solve`) is offered after two tries.
5. **A reason to care** on reading steps: say what the feature saves, in the person's own work words.

6. **Stands alone and is verified.** Two findings from Tadjiro's own run of the lessons: "press R, it turns
   a quarter" was refused by the editor ("it would run along another wire"), and "your branch should look like
   the finished one" did not say which finished one. So:
   - **Replay gate.** CI replays every `ready` and `early` lesson headless from its own start project: each step's
     `solve` actions run through the real command path and the step's `expect` must tick, in order. A step
     whose instruction the program refuses fails CI. A step without `solve` is listed as unverified and a
     lesson with unverified action steps cannot be promoted to `ready`. Steps that say "press R" must have
     `solve` or `demo` that performs exactly that key.
   - **Keys come from the program.** A hand-typed key is a bug waiting to happen ("Tap Q, then 5 for the
     motor protection breaker" while the real way is hold Q, then 4). Commands use `{key:command.id}`; a
     favourite digit or a wheel group uses a new `{wheel:symbol-id}` that resolves from the live wheel and
     favourites, and the replay gate drives the wheel the way the text says (hold, then digit) and fails
     when the symbol that lands is not the one the step names. The lint rejects a bare digit or key name in
     step text outside these markups.
   - **Wording pass.** Text must stand alone: describe the expected result concretely ("the supply and the
     neutral are joined by two wires"), never "like the finished one". Where a picture helps, the last step
     offers **Compare with the finished result**: the lesson's `finished.json` opens in a second tab, or as
     a ghost overlay on the page. Lines that point at something outside the card are rewritten.
   - The lesson threads do a full wording pass over all 33 lessons, First hour path first; the lint flags
     the phrases "the finished one", "as before", "as above", and an `expect` with no `demo`.

Who changes what: the runner (the `demo` field, the demo player, the lint rule that a step with an `expect`
and no `demo` is an error for `ready` and `early` lessons) belongs to the Learning modules thread. The
content threads rewrite their steps to the standard. This plan sets the order they do it in: First hour
path first, then the rest in track order.

## 4. Menus that teach

Principle: the program always knows the next lesson, and every place that has a lesson says so.

1. **Help menu** (a top-level menu in the bar; in the mark's menu when narrow), built from the command
   registry like the other menus:
   - Continue lesson / Next lesson (names it, shows minutes)
   - Learn... (opens the Learn window)
   - Learning paths, then one entry per track with `done/total` (opens the window at that track)
   - Keyboard shortcuts `?` (DISC-1), Fast keys walk-through, Open example, Show hints again, What's new
   - Documentation, About
2. **Learn window**: Continue card on top (current or next lesson, a progress ring); paths; tracks as
   collapsible sections with `done/total` and total minutes; each row shows level, minutes, a "Do first"
   line with the unmet prerequisites as links, and the badge. Search covers title, summary and topics in
   both languages. Empty and first-run states explain what to press.
3. **Start panel**: one line "Start with: First hour" when no lesson is done, "Continue: <lesson>" after.
4. **Feature to lesson links**: a lesson's `topics` already exist. Panels, drawers and the shortcut sheet show
   a small "Lesson: <title>" chip for the topics they know (terminals drawer: terminals; wire list: wire
   numbers; plate view: cabinet layout). Added where a panel has an empty state, as DISC-2 does for tips.
5. **Palette**: Ctrl+K finds lessons by title and topic ("lesson wheel") and the commands `help.learn`,
   `help.next_lesson`, `help.paths`.
6. **After a lesson**: the last step offers the `next` lesson and the matching "Open finished version".

Findability rule from discoverability.md applies: a feature PR that adds a topic also names its lesson.

## 5. Order of work

| PR | What | Where the user finds it |
| --- | --- | --- |
| 1 | This plan, the six tracks, order and `needs` for all 33 lessons, the `early` status | Learn window: every lesson visible with an "Early access" badge |
| 2 | Learn window structure: Continue card, track progress, prerequisites, badge, levels | Start panel and Help, Learn |
| 3 | Help menu: Next lesson, tracks with progress, paths | Menu bar, Help |
| 4 | Paths (`paths.toml`) | Learn window, top |
| 5 | "Lesson:" chips on panels | Terminals, Cables, Wire list, Cabinet drawers |
| 6 | Ctrl+K lessons and `help.next_lesson` | Ctrl+K |
| runner | `demo` field, demo player, lint (Learning modules thread) | Every lesson card, "Show me" |
| content | Steps rewritten to the standard, First hour first | Learn, First hour |

## 6. Open points

- Which lessons carry real GAPs (NOTES.md in ten folders) stay `early`; the content threads close the GAPs
  or the lesson says honestly what the program cannot do yet.
- Advanced level has no lesson yet; first candidates are macros with variants and a full project walk-through
  (draw, number, cable, issue).
