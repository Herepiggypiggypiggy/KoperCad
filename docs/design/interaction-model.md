# Interaction model

Design round 1, written by the workflow thread. It covers what the user
manipulates, how intent turns into model changes, the main journey from a
blank project to a finished panel drawing, where commands live, and which
conventions we reject. The layout thread's `screen-architecture.md`
covers the window and the visual language. Both are critiqued by each other
and by the reviewer, then merged into `docs/DESIGN.md`.

NovoElectric is not EPLAN with fewer clicks. Tadjiro's EPLAN complaints are
used as symptoms: each one shows where page-first CAD breaks down. They are
not a feature list.

## 1. What the user thinks about

- **Functions, not sheets.** "Feed pump 3, 4 kW, direct on line, running
  signal to the PLC" is the unit of thought. A page is how a function is
  printed.
- **Four questions, all day.** What connects to what (connectivity), where
  things sit (cabinet, field, desk), what each thing is (part), and how the
  drawing reads (layout). Page-first tools put all four onto one sheet and
  then need a separate navigator for each.
- **Repetition with variation.** Twenty motor feeders that differ in power
  and tag. Copying frozen macros and renaming by hand is where the hours go.
- **Drawing is thinking.** Placing and connecting must stay direct and
  immediate. A form is not a pen.
- **Big edits are data edits.** Renaming 400 terminals is a spreadsheet
  task, not 400 dialogs.

## 2. What the user manipulates

The user works with five kinds of things. Everything else is derived.

| The user manipulates | Examples | Mostly through |
| --- | --- | --- |
| **Structure** | `=A1 Supply`, `=A2 Pumps`, `+CAB1`, `+FIELD` | The outline, the command field |
| **Devices** | `-K1` with its coil and contacts, `-M1` | Placing on the drawing, the inspector |
| **Connections** | `-K1:2` to `-F1:1` | Dragging from pin to pin |
| **Rules** | Numbering, terminal joining, text style, colours | The Rules page, written as sentences |
| **Blocks** | "Motor feeder DOL" with power as a parameter | The command field, the inspector |

The program derives these, and the user never places them one by one:

| Derived | From | Automatic or proposed |
| --- | --- | --- |
| Wire routes, corners, T-junctions, junction dots | Connections and symbol positions | Automatic; a junction dot can be dragged |
| Potentials (L1, L2, L3, N, PE, 24 V) | The source that declares them, flowing through conductors, contacts and terminals | Automatic; PE, N and PEN are also recognised from names such as `X4:PE` |
| Cross references and contact images | Which device a contact belongs to | Automatic |
| Terminals and cables | Connections that cross a location boundary (`+CAB1` to `+FIELD`) | **Proposed**, accepted with Enter |
| Parts for block devices | Block parameters (4 kW gives a motor protection breaker range) | **Proposed** where more than one part fits |
| Terminal, cable and report pages | The model | Automatic and live |
| Problems | The model and the rules | Automatic, shown in place |

**The line between automatic and proposed:** anything with exactly one
correct answer happens on its own (a junction, a reference, a potential
flowing through a closed conductor). Anything that needs judgement (which
strip, which cable type, which part, where a page breaks) is proposed: it
is drawn ghosted where it applies and accepted with one key. Proposals
never block work, and they never become dialogs.

## 3. How intent reaches the model

```
gesture or typed command
  -> one novo-core Command (one undo step)
    -> model
      -> derivation (routes, potentials, references, proposals, problems)
        -> every view redraws in the same frame
```

- Every gesture is one `Command` and one undo step, including a renumber of
  400 tags or accepting all proposals on a page.
- Derivation never writes to the model on its own. Derived things are
  computed from the model, so undo and scripts see one source of truth.
  Accepting a proposal is the user's command.
- Scripts (WS-28) and the AI assistant (WS-23) use the same commands, so
  anything the user can do by hand can be automated and undone.

## 4. Journey: from a blank project to a finished panel drawing

The story of one small project (a supply and three pumps) told as the user
lives it, with a budget per step. An action is one click, one key press,
one chord or one drag; typed words are listed separately. A budget is a
ceiling; anything slower fails review.

| Step | What the user does | Actions | Typed |
| --- | --- | --- | --- |
| 1. Start | Ctrl+N, name, Enter. The project opens on its structure: `=A1 Supply`, `=A2 Loads`, `+CAB1`, `+FIELD` from the template, plus one empty page under `=A1`. | 3 | name |
| 2. Supply | Command field: `supply 400V 32A`, Enter, click to place. Incoming terminals, main switch and PE bar arrive with "3~ 400 V + N + PE" declared at the source. | 4 | `supply 400V 32A` |
| 3. Three feeders | `dol 4kW x3`, Enter, click. Three tagged feeders (`-Q1..3`, `-K1..3`, `-F1..3`, `-M1..3`) with parts sized, each under `=A2`. | 3 | `dol 4kW x3` |
| 4. Field devices | Ctrl+3 (Devices), select the three motors, set location `+FIELD` once in the inspector. | 4 | `+FIELD` |
| 5. Terminals and cables | Back on the schematic, nine terminals on `-X1` and three cables appear as proposals; `accept all`. | 2 | `accept all` |
| 6. Control by hand | Place two push buttons and a lamp (3 placements), connect five pins (5 drags). | 14 | `pb`, `lamp` |
| 7. Parts left over | The problems count shows "2 devices without a part"; Ctrl+T on Devices, pick the part once for both. | 4 | part search |
| 8. Cabinet | Ctrl+4, `place all`, review the preview, Enter. Warnings appear before placement if a rail overflows. | 3 | `place all` |
| 9. Output | Problems at 0, `export pdf`, Enter. | 2 | `export pdf` |
| **Total** | | **39** | |

The same project drawn device by device, with every wire by hand, is about
120 actions in this model and several times that with page-first tools.
Blocks, derivation and proposals carry the difference.

The Gate 1 benchmark (one DOL starter over two pages) is 54 actions by hand
(placing 9 devices, 17 drags, a supply declaration, one proposal accept,
structure and pages) and 11 with a block.

## 5. Supporting journeys

### Understand a circuit

Selecting or hovering a connection lights up its whole potential, across
pages: the off-page parts are listed in the inspector under **Relations**,
each one clickable. Selecting a device lights up everything it switches on
the page. G goes to the partner (coil, contact, off-page arrow), Shift+G
lists every occurrence, Alt+Left returns. Budget: 0 actions to see, 1 to
jump.

### Change a tag or the numbering

F2 edits a tag in place; typing `-K5` keeps the function and location of
the page. Renaming a coil renames its contacts. A duplicate is refused while
typing, with the reason. The numbering rule is either per letter code
(`-K1`, `-K2`) or per page within the 81346 product aspect (`-K301` for the
first contactor on page 3). Renumbering previews old and new tags and is one
undo step.

### Edit text

Clicking a text selects only the text; clicking the symbol selects the
device. Drag moves it, R rotates it, F2 edits it, and the inspector's reset
mark puts it back where the rule says. The text stays attached to its
device.

### Bulk edit

Every view flips to a table with Ctrl+T, on the same objects and the same
selection: sort, filter, fill down, multi-cell paste, and find and replace
with patterns (`X2:*` to `X3:*`). Selecting 12 contactors on the drawing and
changing their part in the inspector also works and is one undo step.

### Edit a generated view

The terminal and cable views are places where the user edits, not reports.
Reordering terminals on a strip, adding a spare or a jumper, or swapping a
cable core changes the model, and the schematic follows. An edit that
cannot be expressed in the model (moving a terminal's drawn position on a
report page) is a layout override, marked as such.

### What changed

Since the last export or revision, changed objects are marked on every view.
The command `changes` lists them, and each row jumps.

### First run and empty states

A new user sees the structure of a new project and one line on the empty
page: "Type what you are building, or press I to place a symbol". Typing
`dol 4kW` there works the same as in the command field. Every empty view
says in one line how to fill it.

## 6. Where commands live

Every command lives in exactly these places, in this order of preference:

1. **Direct manipulation** for the most frequent things. Drag from a pin to
   connect. Drag a text, a symbol or a junction dot to move it. Drop a
   contact on a coil to give it its device. Drag a device onto a location in
   the outline to move it there.
2. **The command field**, one field for searching and running. A noun
   selects or jumps (`K1`, `12`, `X2:1`), and an exact tag always returns
   that device first with every location. A verb with a noun acts
   (`no K1`, `dol 4kW`, `renumber page`), and a bare verb runs a command
   (`rules`, `export pdf`). Results preview on the drawing before Enter,
   and each row shows its key, so the field teaches the keys. **Every
   command is reachable here.**
3. **The context menu**, built from what is under the pointer. On an object
   its first items are the actions for that object, best guess first (on
   three phase conductors without a source: "Declare 3~ 400 V"). On empty
   canvas its first item repeats the last tool.
4. **Keys** for a small set of frequent actions (below). Letter keys act
   only while the drawing has focus and never take typing from a text
   field.
5. **The inspector** for properties of the selection: Relations first, then
   the few fields that matter for that kind, then a search box over all
   other properties.

Nothing lives only in a menu, and nothing needs a modal dialog in the
drawing loop.

### Keys

| Key | Action |
| --- | --- |
| Ctrl+K | Command field (Space tapped on the drawing does the same) |
| Space held + drag | Pan (also middle drag) |
| Ctrl+1 to Ctrl+5 | Schematic, Connections, Devices, Cabinet, Reports |
| Ctrl+T | Drawing face / table face of the current view |
| I | Place: the command field in place mode at the cursor |
| W | Connect, for chains (one connection is a drag from the pin) |
| R / X | Rotate / mirror while placing or on the selection |
| F2 | Edit the selected tag or text in place |
| G / Shift+G | Go to partner / every occurrence |
| Alt+Left / Alt+Right | Back / forward through jumps |
| Enter | Accept the highlighted proposal; otherwise repeat the last tool |
| Esc | Cancel the step, then leave the tool, then clear the selection |
| Page Up / Page Down | Previous / next page |
| Ctrl+D | Duplicate with the next free tags |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| 1 to 9 | Favourite symbols, only while the place field is still empty |

Favourites act only on an empty field, so typing `m3` or `13` still searches.
There are deliberately no keys for corners, T-pieces, terminals, cables or
potentials, because the user does not place those.

## 7. What we reject, and why

| Convention | Seen in | Why we reject it |
| --- | --- | --- |
| The page is the source of truth | EPLAN, AutoCAD Electrical | It is the root cause of navigators, manual linking and build steps. Pages are views of the model. |
| A navigator per object type | EPLAN | One outline that follows the current view answers "where is it" for every type. |
| Insert centre and symbol trees | EPLAN | Finding a symbol is a search; the command field ranks short names, blocks and recents. |
| Frozen macros | EPLAN, most CAD | They go stale the moment they are placed. Blocks stay linked and parametric. |
| Corner and T-piece symbols | EPLAN | They are drawing artefacts of connectivity, not things the user means. |
| Linking cross references by hand | EPLAN | References follow from which device a contact belongs to. |
| Assigning a potential wire by wire | EPLAN | The source declares it once and it flows. |
| Tabbed property dialogs | EPLAN, Windows CAD in general | Most questions are "what is this connected to", which Relations answers first; the rest is a search. |
| Build, update or check runs | EPLAN | Everything derived is live; problems show in place. |
| Ribbon toolbars | AutoCAD Electrical, Office | Two hundred buttons nobody reads; the command field and a few keys do the job. |
| Jargon command lines | AutoCAD | We keep the speed of typing, with plain words and live previews instead of memorised abbreviations. |
| An infinite canvas | Figma, Miro | IEC documentation is printed and referenced by page and column, so pages stay, but as views. |
| Modal wizards for new projects | Most CAD | A new project is a name and Enter; the rest is defaults and rules. |

## 8. Bold alternatives where the answer is not obvious

### 8.1 Pages

- **A. Hand-made pages.** The user decides what goes on each page, as today.
  Familiar and predictable, but page housekeeping stays a chore.
- **B. Flowing pages.** The user lays out circuits per function; the program
  paginates, places off-page arrows and keeps column references stable.
  Least work, but surprising when a page break moves.
- **C. Pinned flow (recommended).** Pagination is automatic like B, but any
  page break the user sets is pinned and never moves. New circuits flow into
  the next free space under their function.

### 8.2 Routing

- **A. Manual.** The user draws every bend. Full control, slow.
- **B. Always automatic.** Routes recompute on every move. Fast, but fights
  the designer's own tidy layout.
- **C. Route once, then keep (recommended).** A connection is routed when it
  is made, then kept as drawn. Moving a symbol stretches its connections
  orthogonally; `tidy` re-routes a selection on request.

### 8.3 Where a project starts

- **A. Page-first.** A new project opens on page 1. Familiar from every tool.
- **B. Structure-first (recommended).** A new project opens on its structure
  (`=A1`, `+CAB1`); pages appear under functions as circuits are added. It
  matches how designers think and makes 81346 tags free.
- **C. Intent-first.** A new project opens with one question, "What are you
  building?", and `supply 400V, 3 pumps 4kW DOL, PLC` seeds the structure
  and blocks. Powerful once blocks are rich; offered as the empty-page line
  in B rather than as a separate wizard.

## 9. Consistency notes from the critique so far

- **Context menu:** object actions first on an object, "repeat last tool"
  first on empty canvas (reviewer).
- **Favourites:** digits act only while the place field is empty (reviewer).
- **Per-page numbering:** `-K301`, not `-3K1`, to stay valid 81346 and keep
  letter-code numbering working (reviewer).
- **View switching:** Ctrl+1 to Ctrl+5 switches views; panel toggles are
  commands (layout thread).
- **Pan:** holding Space pans, tapping Space opens the command field
  (layout thread).
- **Inspector:** contextual. With nothing selected it shows the page or the
  view. It never reflows the canvas (reviewer).
