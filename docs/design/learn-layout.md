# Learning: menus, panel, chip and the Show me look

Status: spec for review. Owner: Layout and branding. Content and curriculum belong to the
"Structured lesson plan and learning menus" thread; the lesson engine belongs to the Learning
framework thread. This page says where things sit and how they move; it adds no lesson text.
Builds on `docs/design/learning-modules.md`, `docs/design/workspaces.md` section 5 and
`docs/design/ui-guidelines.md`.

## Problem

Tadjiro opened "placing parts fast". The card says what to do, not how. "Show me how" only makes
an area flicker. That is a highlight, not a demonstration. And the menus give no structure for
learning: Help holds Walk-through, Shortcuts and About.

Three layout answers: a Learn menu that reads as a course, a Learn panel that shows the path, and
a Show me that performs the step.

## 1. Learn menu

Learn becomes the ninth menu, between Tools and Help. (`MENUS` in `novo-workflow/src/menus.rs`
grows by one; the "eight menus" text in workspaces section 5 changes with it.) Wide windows draw it
as a word in the bar; narrow ones fold it into the mark menu like the others.

| Entry | What it does |
| --- | --- |
| **Continue: {lesson title}** | Resumes the lesson in progress, else starts the next one on the path. Disabled with a tooltip when every lesson is done ("All lessons done"). Key: none, it is the first row |
| **Learning tracks ▸** | One submenu per track in `tracks.toml` order. Each track row reads `Track name  3/7`; inside, one row per lesson with a state mark (done, current, not started) and the lesson title |
| **Open the Learn panel** | Opens the panel of section 2 (`Ctrl+Shift+L`, to be checked against the key table) |
| **Fast keys walk-through** | The existing walk-through (`help.walkthrough`) moved here |
| **Open example** | The same command as File > Open example, so a learner finds it from here |
| **Keyboard shortcuts** | `view.shortcuts`, moved here from Help |
| **Show hints again** | Brings back dismissed tips (`discoverability.md`) |
| **What's new** | Opens the newest entry of the changelog, when one exists; else hidden |

Help keeps **Search or run (Ctrl+K)**, **About KoperCAD** and **Report a problem** (when one exists).
Rules from section 5.1 hold: every row is a command in the registry, every label is unique in its
menu, a disabled row stays visible and says why. Track and lesson rows are generated from the
catalog, so they are the one place the menu is data and not a fixed list; the test for
"every command has a menu" skips them and a new test checks that every lesson in the catalog has
exactly one row.

Progress in words, never colour alone: `3/7` on tracks, a check mark glyph on done lessons, a copper
dot on the current one. Dutch twins come from `ui.toml`.

## 2. Learn panel

A panel in the Outline's family (left, 280 px, collapsible, same header and section label style),
tab name **Learn**, opened from the menu, the start panel, or Ctrl+K "learn". It is not a window: a
learner keeps it beside the canvas while a lesson runs.

- **Continue card** at the top: lesson title, one line of what it teaches, a bar with step count
  (`Step 4 of 9`), a primary button **Continue** (copper, the only copper in the panel) and a text
  button **Restart**. With nothing started the card says **Start here** and names the first lesson.
- **Path**: below the card, the tracks as a vertical path. Each track is a section label with its
  count (`PLACING AND WIRING  3/7`). Lessons are rows with a state glyph on a rail running down the
  left (filled for done, ring for current, empty for not started), the title, and the minutes on
  the right (`~5 min`) when the lesson has them. A locked lesson (needs an earlier one) is dimmed
  and its tooltip names the lesson to finish first; it still opens, because forcing order
  frustrates people who know the answer.
- **Filter**: a single search field above the path matches lesson titles and the glossary terms
  the lesson teaches. No tabs, no chips; the path is the structure.
- Empty and error states: no catalog loaded shows "Lessons could not be read" and the file path
  in the tooltip; never a blank panel.

Width follows the Outline rules (`workspaces.md` 3.x). At Compact windows the panel is a drawer
over the canvas and closes when a lesson starts, so the card and the canvas have the room.

## 3. Lesson chip

A small chip that links a panel or screen to the lesson that teaches it, by topic.

- Look: 20 px high, pill, `BG_3` fill, `TEXT_SECONDARY` label `Lesson: Rails and ducts`, a small book
  glyph from the icon set, 1 px border at rest. Hover raises the fill one step and shows the
  tooltip `Opens the lesson (~5 min)`. Not copper: the chip is quiet help, not an action.
- Where: in the header row of a panel or workspace, right-aligned, at most one chip per panel.
  Cabinet header, Terminals drawer, Cables drawer, Reports, Commissioning, 3D, Problems. A panel
  with no matching lesson shows nothing.
- Behaviour: click starts the lesson (or resumes it). When the lesson is done the chip reads
  `Lesson done` with the check glyph and a click replays it. Dismissing it from the context menu
  hides it for that panel (a setting, restored by Show hints again).
- Link: the chip asks the catalog for the lesson whose `topics` contain the panel's topic key.
  Lessons gain an optional `topics = ["cabinet", "rails"]` list. The panel keys are fixed in this
  page's table at the end; a lesson may list several, a panel has one.
- Never in a narrow header: when the header has no room the chip moves into the panel's overflow
  menu as a row `Lesson: ...`. A test measures the free room, as for the top bar.

## 4. The Show me look

The aim: the person sees the step done once, by hand, at a speed they can follow, and then does
it themselves. Two parts. The data (what the demonstration does) belongs to the framework, the
look and placement to this page.

### What the framework supplies

A step may carry a `demo` list, in order, each entry one of:

| Entry | Meaning |
| --- | --- |
| `move = <target>` | Pointer travels to the target (a control, a pin, a spot on the page) |
| `click` / `double_click` / `right_click` | Press at the pointer |
| `drag = { from = <target>, to = <target> }` | Press at `from`, travel, release at `to` |
| `key = "Q"` or `keys = ["Ctrl", "K"]` | Show the key press |
| `type = "-K1"` | Text appears in the focused field, letter by letter |
| `wait = 0.6` | A pause, seconds |

Where a step has no `demo`, Show me falls back to what the step already knows: `point` and `note`
become `move` then a click with the check's own meaning. A lint warns (not fails) when a step has
neither, because that is the flicker problem.

The demonstration runs on a **ghost** of the result: the engine applies the step's real command
into a scratch copy of the project, not the learner's. When it ends the ghost fades and the
learner's project is untouched. The learner then does the step for real and the check ticks it.

### The look

- **Pointer**: a 20 px arrow in `TEXT_PRIMARY` with a 1 px `BG_0` outline so it reads on both the
  page and the dark canvas, plus a 28 px copper ring that follows it (alpha 0.35). A press shrinks
  the ring to 14 px for 90 ms and leaves a 36 px ripple that fades in 300 ms. Right click draws a
  second ring; double click ripples twice.
- **Motion**: ease-in-out, 600 ms for a move up to 400 px and 40 ms more per further 100 px, capped
  at 1200 ms. A 250 ms pause before each press and 400 ms after, so the eye can land. Drag goes
  along a gentle arc, not a straight line, and the ghost follows the pointer.
- **Keycap overlay**: keys show as keycaps (`BG_3` fill, 1 px `BORDER`, 6 px radius, `Inter`
  semibold 13 px) in a row near the pointer, 24 px below-right of it, staying 600 ms after the
  press and pulsing once on the press. A chord shows as `Ctrl + K` with the plus as a quiet glyph.
  The overlay never sits on top of the target rect.
- **Ghost preview**: the not-yet-real result is drawn at 50% alpha with a dashed copper outline.
  It is what the learner will have when the step is done; it stays after the demo ends until the
  learner acts or presses Esc, so the target is visible while they do it themselves.
- **Captions**: one line of 13 px text in a small chip (`BG_2`, copper left rule) under the card
  title, changing with each demo entry: `Move to the dock`, `Press Q`, `Drag to the rail`. Words
  come from `ui.toml` with Dutch twins; they describe the action, not the lesson.
- **Controls**: the card gets a row while a demo runs: **Pause / Play**, **Replay**, **Slower**
  (0.5x), **Stop**. `Space` pauses, `R` replays, `Esc` stops. The demo never starts by itself;
  it runs when Show me is pressed, and Replay is always one press away.
- **Reduced motion** (`settings.reduce_motion`): no travel. The pointer jumps between points,
  each held for 1.2 s with its caption, and a **Next** button advances. Same information, no
  animation.

### Never cover the target

The card and the overlay avoid the step's target rects with a placement rule, tested:

1. Candidates for the card: the four corners of the canvas area, then the sides, in the order
   bottom-right, bottom-left, top-right, top-left (the card's current spot first, so it does not
   hop between steps).
2. A candidate is rejected when its rectangle, grown by 16 px, intersects any target ring or the
   straight line between the pointer and its next target.
3. The first accepted candidate wins; if all are rejected, the card collapses to its title row
   (about 44 px high) at the same spot and the test checks that this row is clear too.
4. The keycap overlay and caption use the same test against the target rects; a rejected position
   flips to the other side of the pointer.

A test lays out every lesson's targets at 1366 x 768 and 1100 x 700 and asserts that no card,
keycap or caption rect intersects a target rect. This is the property Tadjiro cares about.

## 5. Panel topic keys

| Panel | Key |
| --- | --- |
| Schematic canvas | `schematic` |
| Cabinet | `cabinet` |
| Rails and ducts (Cabinet) | `rails` |
| Terminals drawer | `terminals` |
| Cables drawer | `cables` |
| Reports | `reports` |
| Commissioning | `commissioning` |
| 3D | `view3d` |
| Problems | `checks` |
| Parts library | `parts` |

## 6. Build order and owners

1. This page merged (Layout). Review by the lesson-plan and framework threads.
2. Framework: `topics` on lessons, `demo` list on steps, per-track progress counts from the catalog.
3. Layout: Learn menu rows and the Learn panel, the chip widget in `novo-app` (new files, hooked
   from the menu bar and panel headers).
4. Layout: demo player (pointer, keycaps, ghost, captions, controls, placement rule) in a new
   `learn_demo.rs`, driven by the framework's step data.
5. Curriculum thread: writes `demo` and `topics` into the lessons.

Each PR names where the user finds it: the Learn menu, the Learn panel, the chip in the panel
headers, and Show me on the lesson card.
