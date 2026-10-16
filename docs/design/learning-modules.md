# Learning modules: lessons that run inside the program

Status: proposal for review. Nothing here is built yet. Thin UI over existing commands, as
`docs/DESIGN.md` asks. Owner: the Learning modules thread. Builds on, and does not repeat,
`docs/ONBOARDING-DESIGN.md` (example project and fast-keys walk-through, PR #428) and
`docs/design/discoverability.md` (Help menu, tips, hints, PR #434).

## Why

The fast keys are the product's point and a text page does not teach them. A person learns this
program by doing: the pointer shows where, the card says why, and the step ticks when the person
really did it. The fast-keys walk-through (#428) is one such lesson hard-coded in Rust. This page
turns it into a system: a **lesson** is a data file plus a sample project, any thread can add one
without touching code, and one engine runs them all.

Goals

1. A **Learn** section, found from the Help menu, the start panel and Ctrl+K.
2. Lessons made of **steps**. A step has a card, a **pointer** on the real control or canvas spot,
   optional **notes** (tooltip-style callouts) and a **check** that ticks when the person does it.
3. Each lesson opens **its own sample project**, through the Open example path (#394).
4. Progress saved per user, locally. Restart, skip, back.
5. A lesson is a folder of **plain text** in the repository. Content threads add lessons by adding
   folders, and CI tells them if a lesson is broken. No cloud session needs a window to write one.

Not goals: video, animated overlays, a web player, telemetry (nothing leaves the machine), Dutch
text in v1 (the format has room for it, see "Language").

## The pieces

| Piece | Where | What it is |
| --- | --- | --- |
| Lesson files | `assets/lessons/<lesson-id>/` | `lesson.toml` and `project.json`; text only |
| `novo-learn` crate | `crates/novo-learn` | Pure engine: parse, validate, step runner, check evaluation, progress. Depends on `novo-core` only (no egui, no editor). Needs registering by the coordinator in its own small PR first, as CLAUDE.md requires |
| Learn shell | `crates/novo-app/src/app/learn.rs` | The Learn window, the lesson card, the pointer overlay, the event funnel, the anchors |
| Lint and replay tests | `novo-learn/tests` and `novo-app/src/app/learn_tests.rs` | Every lesson in `assets/lessons` is parsed, checked against the command and anchor registries, and replayed headless |

The engine is pure so it is testable without a window and so content can be linted in CI.

## 1. Lesson folder

```
assets/lessons/
  tracks.toml                 tracks and their order (framework owns; rarely changes)
  glossary.toml               terms for {tip:...} (append-only; one table per term)
  fast-keys/
    lesson.toml
    project.json              the sample project for this lesson
  wiring-modes/
    lesson.toml
    project.json
```

- `project.json` is the existing diffable project form (`Project::to_json`, `novo-core/src/json.rs`,
  validated on load). A content thread builds the starting project in the app or from a test
  builder, saves it, and runs `kopercad-cli json file.kopercad > project.json`. Keep it under 200 KB.
- A lesson may reuse another lesson's project with `project = "../wiring-modes/project.json"`, or
  use `project = "@example"` (the built-in motor starter, same builder as Open example) or
  `project = "@blank"` (a fresh project with one empty page, like the practice project in #428).
  Sharing is for lessons that really continue the same drawing; each lesson normally owns its file.
- Lessons are **embedded in the binary** by `novo-app/build.rs`, which scans `assets/lessons/*/`.
  Adding a folder needs no Rust edit. Windows build and experimental builds need nothing extra.
- A person (or a trainer) can add lessons without admin rights in
  `%APPDATA%\KoperCAD\lessons\<id>\` with the same layout; they are listed as "Yours" and an id that
  matches a built-in one is refused with a message in the Learn window. That folder is untrusted
  data: `catalog::read_dir` does not follow symbolic links, skips files over 64 KB (text) or 2 MB
  (projects) and folders over 2000 files, reads nothing outside the folder, and names every skip.
  A `project` path is relative and resolves inside the lessons folder; `..` may reach a sibling
  lesson but never out of the folder, and absolute or drive paths are refused.

## 2. `lesson.toml`

```toml
format = 1                      # lesson format version; unknown versions are listed as "needs a newer KoperCAD"
id = "wiring-modes"             # folder name, lowercase, hyphens; never changes once shipped
version = 1                     # bump when steps change meaning; see "Progress"
status = "ready"                # ready | draft (drafts are hidden unless Preferences > Show draft lessons)
title = "Corners and Lines"
title_nl = "Hoeken en lijnen"   # optional, any text field takes a _nl twin
summary = "Draw the same wire two ways and pick the one that suits your hand."
track = "drawing"               # a key in tracks.toml
order = 20                      # sort inside the track; leave gaps of 10 so threads do not collide
level = "beginner"              # beginner | intermediate | advanced
minutes = 6
topics = ["wiring"]             # panels may offer "Lesson: ..." links by topic
needs = ["fast-keys"]           # lessons worth doing first (advice, never a lock)
next = ["potentials"]           # offered on the last step
project = "project.json"        # or "@example", "@blank", or a relative path
finished = "finished.json"      # optional: the same drawing at the end ("Open finished version")
page = "Main circuit"           # page shown on open (by name)

[[step]]
id = "pick-tool"                # stable; progress is kept per step id
title = "Pick the wire tool"
text = """
Press {key:tool.connect}, or click **Connect** in the dock.
Click empty canvas to cancel any time.
"""
point = { ui = "dock.connect" }                  # the pointer (section 4)
expect = { tool = "wire" }                       # the check (section 5)

[[step]]
id = "first-wire"
title = "Wire the contactor to the fuse"
text = "Click the {tip:pin} **-F1:2**, then **-K1:1**. The wire gets its {tip:potential} name for you."
point = { at = { device = "-F1", pin = "2" } }
note = [
  { at = { device = "-K1", pin = "1" }, text = "Finish here" },
]
expect = { wire = { from = "-F1:2", to = "-K1:1" } }
solve = [ { wire = ["-F1:2", "-K1:1"] } ]        # used by CI today (section 7); a later "Do it for me" reuses it

[[step]]
id = "toggle"
title = "Switch mode"
text = "{key:wire.mode} switches Corners and Lines. Draw the next wire the other way."
point = { ui = "dock.wire_mode" }
expect = { command = "wire.mode" }

[[step]]
id = "done"
title = "That is all of it"
text = "You can now choose how wires bend. Next: potentials."
pause = true                    # no check: the person presses Next
```

Rules of the format

- TOML only. `[[step]]` tables run in file order. Every text field is plain, with the small markup
  in section 3.
- `id` fields use `[a-z0-9-]`. A step id never changes after it ships (progress is keyed by it).
- A step with no `expect` is a **reading step** and shows a **Next** button. `pause = true` forces
  Next even when there is an `expect` (read, then act).
- Unknown keys are an error in CI, so a typo cannot silently disable a check.

## 3. Step text

One short paragraph, three lines or fewer on the card. Markup, all optional:

| Markup | Shows |
| --- | --- |
| `**bold**` | Strong text, for names of controls and devices |
| `{key:command.id}` | The command's key **from the live keymap**, as a keycap. A rebound key reads right, an unbound one reads "(no key)" and the step must also say how to reach it another way. Never type a key by hand |
| `{ui:anchor.id}` | The control's visible name, so a rename does not break the lesson |
| `{tip:term}` | The word, dotted underline, hover tooltip from `glossary.toml`. Tooltips are the way to explain a word without bloating the step |
| blank line | New paragraph |

## 4. Pointers, notes and tooltips

A step has at most one **pointer** (`point`) and up to three **notes** (`note`, small callouts with
a number marker). Both take the same **target**:

```toml
point = { ui = "dock.connect" }                         # a control
point = { ui = ["mark.menu", "mark.menu.save"] }        # a path: points at the deepest one visible
point = { at = { device = "-K1" } }                     # a device on the shown page
point = { at = { device = "-K1", pin = "A1" } }         # a pin
point = { at = { page = "Control circuit", x = 120, y = 60 } }   # a canvas spot in mm
point = { at = { wire = ["-F1:2", "-K1:1"] } }          # a wire by its ends
point = { at = { pin = "-K1:A1" } }                     # a pin written whole
point = { at = { feed = "L1" } }                        # the potential feed named L1, wherever it is
point = { area = { page = "Main circuit", x = 20, y = 20, w = 80, h = 50 } }  # a region
point = { ui = "dock.connect", view = "keep" }          # canvas targets default to view = "reveal"
```

- **UI anchors.** The app registers a stable string id with the screen rectangle of each control
  every frame (`anchor(ui, "dock.connect", response.rect)`). A lesson names the id. The catalogue
  of ids lives in `novo-app/src/app/anchors.rs` and is generated into
  `docs/design/lesson-anchors.md` (a test keeps them equal). Ids are `area.thing` in lowercase:
  `dock.*`, `mark.menu.*`, `outline.*`, `inspector.*`, `drawer.terminals.*`, `palette.field`,
  `start.*`, `learn.*`. Renaming a control keeps its id.
- **Path targets.** If the target control is inside a closed menu or drawer, the pointer goes to
  the deepest anchor that is visible and the card says "Open the mark menu first" using the next id
  in the path's visible name. The person is never stuck wondering where it went.
- **Canvas targets** use device tags and pin names, never entity ids, so a rebuilt sample project
  keeps working. They go through `novo-canvas` viewport math and move with pan and zoom. With
  `view = "reveal"` (default) the view is moved only if the target is off screen, using the same
  reveal as cross reference jumps; a lesson never changes the zoom of a person who already has it
  in view.
- **Point at what the person made, not at a coordinate.** A page spot (`x`, `y`) is right for "drop
  it here" before anything is placed. Once the person has placed or drawn something, its place is
  theirs: a feed they dragged lands wherever they pressed. Point at it by what it is (`device`,
  `pin`, `feed`, `wire`) so the box is on the thing, whatever they did.
- **Look.** A soft pulsing ring (or rectangle for areas) in the accent colour, plus a short arrow
  from the card when the target is far from it. It never dims the screen, never blocks a click and
  never takes a key (`interactable(false)`). With `reduce_motion` the ring is static. Colours come
  from `novo_theme` tokens; the design-system thread owns the look.
- **Notes** are the in-place tooltips: a numbered marker with a small bubble next to the target,
  shown while the step is active. Use them for "this is the inspector, this is the issue pill"
  steps (reading steps that tour a screen) so one step can explain three places.
- A target that does not exist (symbol not on the page, anchor missing in this build) never panics:
  the card shows the text without a pointer and the engine logs it; CI reports it as an error for
  `ready` lessons.

## 5. Checks: the step ticks when the person does it

A check is a small predicate. Two families, because both are needed:

**Events** say what the person did. They are good for keys, tools and panels.

| Check | True when |
| --- | --- |
| `{ command = "tool.connect" }` | `run_command("tool.connect")` ran (palette, menu, and every key that is a command) |
| `{ signal = "wheel_opened" }` | the shell raised a named signal for something that is not a command: the place wheel opened, Tab took the next proposal (`proposal_next`), Enter accepted one (`proposal_accept`). The names are a registry in `novo-app`, linted like commands |
| `{ tool = "wire" }` | the tool in hand is `select`, `place`, `wire`, `cable` or `text` |
| `{ opened = "drawer.terminals" }` | the anchor became visible (a panel, drawer or menu opened) |
| `{ clicked = "outline.issues" }` | the anchor was clicked |
| `{ page = "Control circuit" }` | that page is shown |
| `{ selected = "-K1" }` | the selection is exactly that device (or `selected_count = 3`) |
| `{ zoom_at_least = 2.0 }` | the view is at least that zoomed in |

**State** says what the document is now. It is robust: it does not care which way the person got
there (key, drag, palette) and survives undo and redo, so it is the right check for "the drawing
now has X".

| Check | True when |
| --- | --- |
| `{ wire = { from = "-F1:2", to = "-K1:1" } }` | a wire (or a routed chain through junctions) joins those pins |
| `{ device = { tag = "-K2", symbol = "iec/contactors/coil" } }` | a device with that tag and/or a symbol of that key exists; give `symbol` alone when the program assigns the tag, and `at_least` to count |
| `{ count = { kind = "symbol", page = "Main circuit", at_least = 4 } }` | that many entities exist; kinds are symbol, wire, device, page, cable, text, junction and feed (feed symbols and pins that define a potential) |
| `{ field = { device = "-M1", name = "part", is_set = true } }` | a property is set or equals a value |
| `{ cable = { name = "-W1", cores_at_least = 4 } }` | a cable exists |
| `{ problem_free = "wire.overlap" }` | the named design check reports nothing (the shell passes the ids that report something in `Facts`, so the engine needs no `novo-checks`) |
| `{ potential = { wire = "-F1:2", is = "L1" } }` | the net of that pin carries that potential |
| `{ potential_kind = { kind = "protective_earth", page = "Motor feeder" } }` | some wire (on that page) is on a net with a potential of that kind |
| `{ wiring_mode = "lines" }` | the wiring mode setting |
| `{ live = true }`, `{ coil_on = "-K1" }`, `{ pole_closed = "-S1" }`, `{ lit = "-K1:A1" }` | the live view is on, a coil is energized, a pole is closed, a pin carries voltage (the shell passes these in `Facts`; "released" is `{ not = { coil_on = ... } }`) |

Anything that is shell or simulation state rather than document state (tool, page, selection,
zoom, wiring mode, live view, design-check results) arrives in `Facts`, filled in by the app. That is
what keeps `novo-learn` dependent on `novo-core` alone: it never calls `novo-checks`, `novo-cables`,
`novo-parts` or `novo-sim`. Cables, parts and potentials are read from the model itself.

Combinators: `{ all = [ ... ] }`, `{ any = [ ... ] }`, `{ not = { ... } }`.

How a step completes

- On each event and after each committed change the engine evaluates the current step's check.
  When it is true the step ticks, a short check mark flashes, and the next step opens after about
  half a second (immediately with `reduce_motion`). Reading steps wait for Next.
- State checks are also tested **on entering** a step. Authors must not write a step that is
  already true at its start. The lint tests this against the sample project: an error for the
  first step, a warning for later ones (an earlier step may legitimately change what is true), with
  the replay test (section 7) as the final word.
- Undo of the work of an already ticked step does not un-tick it (the person has moved on), but
  the current step's own state check turns false again if its work is undone.
- A check names things the person controls (tags, pins, pages). Do not check a tag the engine
  invents (auto tags depend on numbering); check by `symbol` and `count` instead.
- **Honest ticks.** Same rule as #428: a tick means the person did the thing. A change set alone is
  not proof (`added` also fires on paste, duplicate, undo and replay), which is why events come
  from the command funnel and state checks read the model.

New check kinds are an app change (small, in `novo-learn` with a test). `format = 1` lessons keep
working.

**Strict and lenient, stated once.** There is one parser and it is strict: an unknown key, check kind
or `solve` operation is an error, which is what catches a typo in CI. At run time the same error
never hides or crashes anything: the lesson is listed in the Learn window as **Needs a newer
KoperCAD** (for an unknown `format` or an unknown kind, `LessonError::NeedsNewer` and `UnknownKind`)
or **Cannot be opened** with the reason (anything else), and every other lesson works. The lint
decides which it is: a built-in lesson that does not parse is a CI failure, never a user-facing one.

## 6. What the person sees

The **lesson card** sits bottom right of the canvas, in the same place and family as the #428 walk-through
card (only one of the two shows at a time; starting a lesson hides the walk-through). It never
takes a key and never blocks input.

```
 Corners and Lines · 2 of 4                        ✕
 ○ Pick the wire tool
 ● Wire the contactor to the fuse
   Click pins -F1:2 and then -K1:1. A potential is named for you.
   [ Back ]  [ Show me how ]  [ Skip step ]        ⟲ Restart lesson
```

- **Show me how** plays a demonstration over the real screen, repeated up to three times and
  stopped when the step changes: a pointer gliding to the control or the spot, key caps with the
  person's own keys, text typed letter by letter, a click ripple, a faint copy of the part that
  lands and a wire being dragged. The card lists the same actions under **How**, with the one being
  done in bold. It never changes the drawing. With Preferences > reduce motion it shows the last
  picture, still. A **Do it for me** button that runs the step's `solve` as one undo step is not in
  the first version; the format keeps `solve` so it can be added without changing any lesson.

  A step writes its demonstration with `demo`, a list played in order (section 6a); a step without
  `demo` gets one derived from the command that ticks it, its `solve` and its `point`.
- **Back** returns the card to the previous step (it does not undo the drawing). **Skip step**
  moves on without the tick and records "skipped". **Restart lesson** reopens a fresh copy of the
  sample project and clears this lesson's progress after a confirm. ✕ closes the card and keeps
  progress; the Learn window offers **Continue** while the lesson's own project is still open. The
  sample project is not saved between sessions, so in a later session the same lesson offers
  **Start** (or **Do again**) and begins at step 1, with the old progress still shown as steps done.
- Esc does not close the card: Esc belongs to the editor.
- The last step shows "Done", a summary of what was covered, and the lessons in `next`.

The **Learn window** (a centred panel like the start panel, not a modal) lists tracks from
`tracks.toml`; each lesson shows title, summary, level, minutes, a progress ring and one button:
**Start**, **Continue** (with "step 3 of 9"), or **Do again**. A search field filters by title,
summary and topics. A lesson that names a command or anchor this build does not have is shown as
**Coming soon** (greyed, not broken); that is how content can land before the UI it teaches.

Starting a lesson replaces the open project, so it goes through `leave_then` like Open example:
unsaved work gets the usual choice first. The lesson's project opens as an untitled project named
"Lesson: <title>", marked saved, so closing it untouched leaves nothing under Unsaved; the first edit
makes it dirty and Ctrl+S keeps the person's own copy. Nothing is ever written to the lesson files.

### Where to find it (feature-visibility rule)

| Entry | How |
| --- | --- |
| Start panel | A **Learn** button beside Open example; on a first run it is the highlighted one, with a **Continue: <lesson> (step n of m)** line when something is under way |
| Help menu (#434) | **Learn…** opens the window |
| Palette | `help.learn` opens the window; `help.learn wiring` starts the best match; `learn.next`, `learn.back`, `learn.restart`, `learn.close` act on the running lesson |
| Panels | A faint **Lesson: Terminal strips** link in an empty state, from the lesson's `topics` (the design-system thread owns the widget) |
| Shortcut sheet (#434) | Links to Learn |

### 6a. `how` and `demo`: the instructions and the demonstration of a step

`how = ["Hold {key:tool.place}", "Press 4"]` (with `how_nl` of the same length) are the author's
numbered instructions on the card: the keys and clicks, and why. Same markup as `text`; at most 8
lines. When a step has `how`, the card shows them under **How**; when it has none, the card lists the
actions of the demonstration instead. `demo` below is what **Show me how** plays.

```toml
demo = [
  { press = "tool.place" },               # the person's own key for a command (Q), as a key cap
  { type = "mcb" },                       # typed letter by letter
  { key = "Enter" },                      # a key by its name
  { hold = "tool.place", secs = 1.2 },    # a key held down
  { move = { at = { page = "Control branch", x = 80, y = 40 } } },   # a target, as `point`
  { ghost = { symbol = "iec/protection/mcb_1p", page = "Control branch", x = 80, y = 40 } },
  { click = true },                       # a click ripple where the pointer is
  { double_click = true },                # two rings; right_click = true shows the right button
  { keys = ["Ctrl", "Z"] },               # several keys at once
  { drag = { from = { at = { pin = "-K1:A1" } }, to = { at = { pin = "-F1:2" } } } },
  { say = "Release over the pin", say_nl = "Laat los boven de pin" },   # a line of the How list only
  { wait = 0.5 },
]
```

- `press` and `hold` name a command id: the cap shows the key the person has, so a changed key shows
  correctly. The How list says "Press Q" from `demo_press` in `ui.toml` (with a Dutch twin); nothing
  readable is typed in code.
- `move`, `drag` and the pin of a `ghost` use the same targets as `point` (`ui`, `at`, `area`).
  `at = { pin = "-K1:A1" }` names a pin without repeating the device.
- **A wire step shows its two ends.** Every `drag` draws a ring on the start (badge 1, "Start")
  and on the end (badge 2, "End"; a second wire is 3 and 4) and a dashed ghost wire between
  them. The rings leave the pin itself in view and the badges flip inwards at the page edge. After
  the three plays the markers and the ghost wire stay until the step changes, **Stop** is pressed
  on the card or Esc is pressed. A step ticked by `expect = { wire = { from, to } }` gets this
  without writing a `demo`.
- Every `press`, `hold` and `ui` target in `demo` is checked by the lint like the rest of the step;
  in a `ready` lesson an unknown one is an error.
- Without `demo`, the demonstration is: the command of an `expect = { command }`, then each `solve`
  op (a placed part is a move, a ghost and a click; a wire is a drag between its pins; a selection is
  a move and a click), else a move to `point` and a click when it is a control.

## 7. Content threads: how to write a lesson

Rules of thumb (the reviewer checks them)

1. One idea per lesson, 6 to 12 steps, 5 to 10 minutes. Start with what the person will have at
   the end. End with `next`.
2. One action per step. If the text has "and then", it is two steps.
3. **Teach the key first**, the click second. The fast keys are the product. Write `{key:...}` and
   show the control with `point`; do not write "click the button" when a key exists.
4. Plain words, no EPLAN terms or comparisons. Explain a word once with `{tip:...}`.
5. Never teach a feature that is unfinished or hidden. Mark such a lesson `status = "draft"`.
6. The sample project has just enough drawing to make the point, a name that says what it is, and
   illustrative parts (say so in its description, as the example does).
7. Every action step has a `solve`, so the lesson is tested, and a `demo` whenever the action is not
   just a click on what the pointer shows (a key, a typed word, a held key, a drag), so "Show me how"
   teaches *how*; the derived demonstration only points and clicks.

CI (all run in the normal `cargo test --workspace`, Linux, no window)

- **Lint** (`novo-learn`): the TOML parses with no unknown keys; ids are unique and well formed;
  `track`, `needs`, `next` and `project` resolve; text markup is balanced; every `{key:...}`
  command, every `command` check and every `ui` target exists in the registries (`ready` lessons;
  `draft` lessons may name unknown ones); `project` and `finished` load through `from_json`; a canvas
  target that is not in the sample project at the start is a warning (it may be drawn by an earlier
  step; the replay confirms); a state check that already holds at the start is an error for the first
  step and a warning later; the size limits hold. The parse part of this runs as plain
  `cargo test -p novo-learn` (`crates/novo-learn/tests/repo_lessons.rs`), so a content PR fails
  there before the app-side lint even runs.
- **Project format bumps.** `project.json` carries `JSON_FORMAT_VERSION`. A core bump would break
  every lesson at once, so the test above loads each one and names the file; the fix is one
  command per lesson (`kopercad-cli json`, in `assets/lessons/README.md`) and changes nothing else.
- **Replay** (`novo-app/src/app/learn_tests.rs`, gate1-style headless): each lesson's project is
  opened, then every step's `solve` is run in order through the real command path and the step's
  `expect` must become true. If a lesson passes, a person who does the same things gets the same
  ticks. This is also how a cloud thread without a screen knows its lesson works.
- `solve` operations: `{ command = "id", arg = "..." }`, `{ place = { symbol, page, x, y, parent } }`
  (`parent` places it as another function of an existing device, such as a contact image of `-K1`),
  `{ wire = ["tag:pin", "tag:pin"], via = [[x, y]], mode = "lines" }`,
  `{ tee = { from, onto, at = [x, y] } }` (a wire that starts with a T on another wire),
  `{ feed_row = { names = ["L1", "L2"], page, x, y } }`, `{ select = "-K1" }`, `{ page = "name" }`,
  `{ rename = { target = "-K1", to = "-K9" } }`, `{ cable = { ... } }`, `{ issue_revision = "description" }`,
  `{ assign_part = { device, part } }`, `{ key = "x", hold = 0.5 }` (a held key; a pushbutton is momentary,
  so a pressed-and-released click needs the hold), `{ live_pole }`, `{ live_coil }`, `{ set_potential }`.
  New operations are added with the check kinds above.
- **`manual = "why"`** on a step marks it as something the replay cannot do (a file dialog, a printer, a
  real EPLAN). It is an error to give both `manual` and `solve`; an `expect` with neither is a warning, so
  the gap stays visible. The report lists such steps as Manual.
- **Anchors.** Besides the static anchors (`app/learn.rs`, listed in `docs/design/lesson-anchors.md`), every
  menu and every menu row is an anchor: `{ui:menu.File}` and `{ui:cmd.<command id>}`. When a row is folded
  away (narrow window, More menu) the pointer falls back to the menu or button that holds it.
  Write control names in step text with `{ui:...}`, not bold text: `app/learn_labels.rs` holds the label
  table, and `docs/design/lesson-ui-names.md` is a ratchet of bold control names still written as free
  text. The test fails when a lesson adds one; regenerate with `UPDATE_UI_NAMES=1 cargo test -p novo-app
  lessons_name_no_new_control` only after converting, never to hide a new one.

Authoring loop for a thread with no window: write `lesson.toml` and `project.json`, run
`cargo test -p novo-learn -p novo-app learn`, read the report. For someone with the app:
setting the environment variable `KOPERCAD_LESSONS` to a folder loads its lessons on top of the
built-in ones (so does `%APPDATA%\KoperCAD\lessons`), and the Learn window has a **Reload** button, so
edits show without a rebuild.

## Language

English first, but nothing a person reads is typed in Rust. Every lesson string is in the lesson
files: any text field (`title`, `summary`, `text`, a note's `text`, a glossary entry) takes a `_nl`
twin, and the loader picks the app language and falls back to English. The card's own words (Back,
Skip step, Restart lesson, Continue, Coming soon, ...) live in one file, `assets/lessons/ui.toml`,
with the same `_nl` twins, so the Dutch work edits data only. A lint test fails when a `_nl` twin
exists for a field that is missing in English, and reports lessons that lack a Dutch twin as a
count, not an error.

## 8. Progress

Per user, local, never uploaded. One small file, `%APPDATA%\KoperCAD\learn.json`
(`dirs::config_dir()`), not the egui settings blob, so it survives a settings reset and "reset
progress" is a single button:

```json
{ "format": 1,
  "lessons": { "wiring-modes": { "version": 1, "done": ["pick-tool"], "skipped": [],
                                  "current": "first-wire", "completed": "2026-10-15" } } }
```

- Written on each tick, with the same write-then-rename the project autosave uses. A bad or newer
  file is renamed aside (`learn.json.bad`) and a fresh one started; the app never fails to start
  over it.
- If an author bumps `version`, the lesson shows **Updated**; steps whose ids still exist keep
  their ticks, a removed id is dropped, and the person can press Do again.
- The first-run flag from #428 (`Settings.onboarding`) stays where it is. When the walk-through
  becomes the `fast-keys` lesson, its ticked step ids map one to one onto lesson steps and migrate once.

## 9. How it plugs into the app

- **Events come from three places, not one.** `run_command(id)` sees palette, menu and every key that
  is a command. Editor keys do not pass it: W, Tab and Enter on proposals are handled in the editor
  and the workbench, and holding Q reaches `open_wheel`. Those raise **signals** from their existing
  hook points (`proposal_next`, `proposal_accept`, `wheel_opened`), and the tool in hand is a fact
  (`{ tool = "wire" }` ticks when W arms the tool, which is the honest tick #428 uses). Document
  changes arrive through `backend.record(&changes)`. All of them call `learn_event` or
  `learn_update`, thin functions that hand the event or the facts and a `&Project` to `novo-learn`.
  #428's `tour_tick` becomes one call to the same function when the walk-through is converted to the
  `fast-keys` lesson, so there is one detector, not two. Until then this work does not edit
  `onboarding.rs`.
- **Anchors** are one line per control (`anchor(ui, id, rect)`), stored in a frame-local map the
  overlay reads. `anchor` is the low-level piece: the discoverability thread's `tip(response, label,
  command_id)` (#434) takes the anchor id too and calls it, so a control gets its tooltip, its
  shortcut and its lesson anchor in one call and there is no second tooltip convention. Feature threads add them for their own controls; the rule joins the findability
  rule of #434: *a control or panel a lesson could point at registers an anchor id*.
- **The overlay** is an `egui::Area` with `interactable(false)` at `Order::Foreground`, so it can
  draw over panels without taking a click.
- **Ownership.** `novo-learn` and `learn.rs`: this thread. `assets/lessons/<id>/`: the content
  thread that wrote it. `tracks.toml` and `glossary.toml`: this thread (append rows by PR; rows are
  independent so they merge cleanly). Anchors: the thread that owns the control. `novo-core` is
  read through queries; no model change is needed (the JSON form already exists).

## 10. Suggested first lessons and who writes them

Four content threads, one track each, listed in `assets/lessons/tracks.toml`. Track titles are the
ones the threads were given; lesson ids below are suggestions (the folder name), and each thread
owns its list.

| Track (`track =`) | Lessons |
| --- | --- |
| **Schematic basics and motor control** (`basics`) | `tour` (screen tour: dock, outline, inspector, pill, Ctrl+K, `?`), `fast-keys` (the #428 steps as a lesson), `first-schematic` (build the control circuit of the motor starter from `@blank`), `wiring-modes`, `proposals-and-tags`, `duplicate-and-numbering`, `pages-and-functions`, `cross-references`, `potentials`, `motor-starter`, `save-and-share` |
| **Terminals, PLC and documentation** (`terminals`) | `terminal-strips`, `connection-screens`, `cables-and-shields`, `plc-io`, `generated-pages`, `wire-list-and-bom`, `pdf-export` |
| **Cabinet layout and panel building** (`cabinet`) | `cabinet-layout`, `enclosure-sizing`, `wire-routing`, `view-3d`, `panel-plate-drawing`, `commissioning` |
| **Productivity, checks and data** (`productivity`) | `design-checks`, `parts-library`, `blocks-and-macros`, `spreadsheets`, `revisions-and-compare`, `dxf-and-backgrounds`, `time-savers` (settings: Auto connections, keymap file, drawing standard). `commissioning` is written by this thread but listed under `cabinet`. `live-view` is left to the simulation owner |

Each lesson starts as `draft` until the control it teaches has an anchor on main, then flips to
`ready` in its own PR.

## 11. Build order (small draft PRs, no build)

1. This page, with the thread's handoff files. Merge first so the content threads can start.
2. Coordinator registers `novo-learn` (small PR).
3. `novo-learn`: format structs, parser, validation, check evaluation, runner, progress store, the
   lint, with the `wiring-modes` and `tour` lessons as test data. No UI.
4. App shell: event funnel, anchors for the dock, mark menu and start panel, the card, the pointer
   overlay, the Learn window, start-panel and Help entries, `help.learn`. One real lesson end to end.
5. `build.rs` embedding, user lesson folder, `--lessons`, Reload, the replay tests.
6. Anchor rollout in the other threads' controls; `docs/design/lesson-anchors.md` generated.
7. Content PRs from the four threads (they can start at step 1 as drafts).
8. The #428 walk-through becomes `fast-keys`; the old card code is deleted.
9. Later: Dutch text, "common mistake" hints (`[[step.mistake]]` with a `when` check and a
   `say`), a lesson badge on panels the person has never opened, an end-of-track summary.

## Coordination

- **Sample project and first-run walk-through** (cse_019sSSXcKhY8k5uYwY3cgSyf): owns #428, the
  example and the practice project. This work reuses their funnel and `open_example` path; the
  walk-through becomes lesson `fast-keys` after #428 merges, converted by them with this engine.
- **Onboarding and discoverability** (cse_01CpKBFnbxA5RuJyh1pGkTdZ): owns the Help menu, tips and
  hints (#434). Learn is one Help item; the shortcut sheet links to it; hint counters stay theirs.
  The anchor rule is added to their findability checklist.
- **Workspaces** (cse_01WAQSEGkMbyreCPG18tsSPc): panels move; anchors keep ids across moves, so
  lessons do not care where a panel lives. They own the stable names of the areas.
- **Design system thread** (section 10 of `docs/design/design-system.md`, #448): builds `pointer`,
  `lesson_card`, `note`, `tip` and `lesson_link` in `novo-theme`. The shell calls them with plain
  data so they stay free of lesson logic: `pointer(ctx, rect, pulsing)` for a ring or area
  (`pulsing` off with reduce motion), `note(ctx, rect, number, text)` for a numbered callout,
  `lesson_card(ui, header, body, buttons) -> Option<CardAction>` where `header` is lesson title
  with step `n of m`, `body` is a closure that draws the step rows and text (the shell renders the
  markup, since keycaps and `{tip:}` need the keymap and glossary), `buttons` says which of Back,
  Show me how, Skip step, Next, Restart lesson and Close are enabled, and `CardAction` names the one
  pressed. `lesson_link(ui, lesson_title) -> bool` is the "Lesson: ..." line in an empty state.
  Until those land the shell draws plain versions from `novo_theme` tokens.
- **Workflow thread**: owns `commands.rs` and the keymap. Adds `help.learn`, `learn.next`,
  `learn.back`, `learn.restart` and `learn.close` with keywords "lesson", "tutorial", "training",
  "learn", "teach".
- **Tester**: owns `novo-tests`; the lint and replay tests live in `novo-learn` and `novo-app` and
  add no dependency there.

## Decisions

Settled with the coordinator: TOML for lessons and JSON for projects; no **Do it for me** in the
first version (the `solve` hook stays); English first with every string translatable (section
"Language").
