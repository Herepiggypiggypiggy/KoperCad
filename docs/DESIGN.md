# NovoElectric design

How NovoElectric works for the person using it, and what its window looks
like. This is the single design reference for UI pull requests. It merges
the workflow thread's interaction model and the layout thread's screen
architecture after two critique rounds with the main reviewer; the
integration lead arbitrates changes. It replaces `docs/WORKFLOW.md` and
`docs/design/interaction-model.md`.

NovoElectric is not EPLAN with fewer clicks. Tadjiro's EPLAN complaints
(appendix) are treated as symptoms of page-first CAD, not as a feature list.
The visual language (colours, type, spacing, icons) is in
`docs/design/ui-guidelines.md`.

## 1. How a designer thinks

- **Functions, not sheets.** "Feed pump 3, 4 kW, direct on line, running
  signal to the PLC" is the unit of thought. A page is how a function is
  printed.
- **Four questions, all day.** What connects to what, where things sit,
  what each thing is, and how the drawing reads. Page-first tools put all
  four on one sheet and then need a navigator for each.
- **Repetition with variation.** Twenty motor feeders that differ in power
  and tag.
- **Drawing is thinking.** Placing and connecting must stay direct and
  immediate.
- **Big edits are data edits.** Renaming 400 terminals is table work.

## 2. What the user works with

The user manipulates five kinds of things. Everything else is derived.

| The user manipulates | Examples | Mostly through |
| --- | --- | --- |
| Structure | `=A1 Supply`, `+CAB1`, `+FIELD` | The structure board, the breadcrumb, the outline filter |
| Devices | `-K1` with its coil and contacts | Placing on the drawing, the inspector |
| Connections | `-K1:2` to `-F1:1` | Dragging from pin to pin |
| Rules | Numbering, terminal joining, text style, colours | The Rules document |
| Blocks | "Motor feeder DOL" with power as a parameter | The command surface, the inspector |

| Derived | From | Automatic or proposed |
| --- | --- | --- |
| Wire routes and corners | Routed once when a connection is made, then stored | Automatic; `tidy` re-routes on request |
| Junction dots and T-junctions | Where stored routes meet | Automatic; a dot can be dragged. A dot means three or more conductors join: a straight run of two connections through a point, a corner and a crossing without a connection get no dot (IEC 60617) |
| Connections between placed pins | Where a placed or moved symbol's pins land | Automatic when pins touch or a pin lands on a connection. Free pins that face each other at a distance are proposed: drawn as proposals in the placement preview, so the click is informed, and still there after the drop for Enter or Tab. Moving a symbol never makes a connection the preview did not show |
| Potentials (L1, L2, L3, N, PE, 24 V) | The declared source, flowing through conductors, contacts and terminals | Automatic. Recognising PE, N and PEN by name (`X4:PE`) is a visible, switchable rule; one conductor can be overridden |
| Cross references and contact images | Which device a contact belongs to | Automatic |
| Terminals and cables | Connections that cross a location boundary (`+CAB1` to `+FIELD`) | Proposed |
| Parts for block devices | Block parameters | Proposed where more than one part fits |
| Terminal, cable and report pages | The model | Automatic and live |
| Problems | The model and the rules | Automatic, marked in place |

**Automatic or proposed:** anything with exactly one correct answer happens
on its own. Anything that needs judgement (which strip, which cable type,
which part, where a page breaks) is proposed: drawn as a proposal mark where
it applies and accepted with Enter. Proposals never block work and never
become dialogs. A declined proposal is remembered and does not return.

**How intent reaches the model:** each gesture or command is one novo-core
`Command` and one undo step, including a renumber of 400 tags or accepting
all proposals. Derived things are computed, never written behind the user's
back. Scripts (WS-28) and the AI assistant (WS-23) use the same commands.

## 3. The window: Workbench

![Workbench](design/layout-a.png)

```
+--------------------------------------------------------------------------+
| mark | Schematic Cabinet | project / =A1 / +CAB1 / page  [Search or run] |  42 px
+---------------+----------------------------------------------------------+
| Pages Issues  |                                      hud  +------------+ |
|       Changes |                                           | Inspector  | |
| [filter =/+ ] |               canvas                      | (overlay,  | |
| RECENT        |                                           |  pinnable) | |
| ALL PAGES     |          [ tools | hint | proposals ]     +------------+ |
+---------------+----------------------------------------------------------+
| Terminals | Cables | Devices   (drawer, linked to the selection)      |
+--------------------------------------------------------------------------+
```

What the screen must do:

1. **The drawing never moves because of chrome.** Panels that appear on
   selection float over the canvas; only pinned panels take space. When the
   drawer opens or closes, the world point at the canvas centre stays put.
2. **One way in for commands:** the command surface reaches every command,
   page, device and symbol.
3. **Information sits where it applies:** problems on the object, the hint
   next to the tool, properties beside the selection.
4. **Quiet until needed:** at rest, the drawing plus one thin bar.
5. **Works on a site laptop:** 1366x768 and 1536x864 effective (1920x1080 at
   125%) are design targets. With the outline as a flyout and the drawer
   closed, the canvas gets 95% of the window.

**Top bar (42 px).**
- The **mark** opens a short project menu: New, Open, Recent, Export PDF,
  Print, Project settings, Preferences, Quit. Every item is also in the
  command surface; the mark is a shortcut list, not a second index.
- **Schematic | Cabinet:** the two canvases, as a segmented switch.
- **Breadcrumb:** project / `=function` / `+location` / page. A click on a
  segment filters the outline to it.
- **Search or run** (Ctrl+K): the command surface.
- **Issue pill:** the issue count; a click opens Issues. Each count lives
  in exactly one place: the pill counts issues, the dock counts proposals.
- **Saved:** the only save signal, reading Saving, Saved or Not saved yet.
  See Saving below.

**Outline (left, 248 px).** Pinned above 1700 px wide, a flyout below;
pinned from its pin button or the "Pin outline" command. Three modes:
- **Pages:** Recent, then All pages. The filter takes text and structure
  (`=A1`, `+CAB1`, `motor`): the 81346 tree filters pages rather than
  owning them, because one page can hold several functions.
- **Issues:** problems and proposals in one sortable list with filter
  chips, worked through with F8 and Shift+F8.
- **Changes:** what changed since the last revision, by page (§7, Revisions).

**Canvas.** Two floating things:
- The **tool dock** (bottom centre): Select, Place, Connect, Text, the hint
  for the current step ("Click the second pin · R rotate · Esc stop") and
  the proposals count. There are no terminal, cable or potential tools. The
  "Compact dock" setting leaves only the hint and the count. It moves aside
  while a gesture is in progress near it (it never fades), and can be
  dragged to any edge.
- The **HUD** (top right, `TEXT_FAINT`, 4.5:1 on the canvas): cursor position in mm, grid, snap, zoom.

**Inspector (overlay, 276 px).** Appears with a selection and slides away
when it clears. It opens on the side away from the selection, flipping to
the left edge when the selection is under it, so it never hides what you
selected. Content: **Relations** first (feeds, supplied by, contacts,
strip, cable, block "DOL feeder 4 kW, 2 overrides"), then the part, then a
search over every other property. Overridden fields show a reset arrow. A
pin makes it a real panel; pinned with nothing selected it shows the page.

**Drawer (bottom, linked).** Three tabs, all tables of the same model with
the drawing still in view:
- **Terminals:** strips, levels, jumpers, spares.
- **Cables:** cables, cores and shields.
- **Devices:** every device with its part and location; on the Cabinet
  canvas, unplaced devices come first and the drawer opens there by
  default.
Selecting on the drawing selects rows and the other way round. Ctrl+3,
Ctrl+4 and Ctrl+5 toggle the tabs. The drawer takes at most 35% of the
height, keeps the canvas-centre world point fixed, docks right above
1800 px wide (layout C) and below 800 px of height.

| Screen (effective) | Outline | Drawer | Canvas |
| --- | --- | --- | --- |
| 1920x1080 at 100% | pinned, 248 | bottom, 30% | 1672 x 714 |
| 1536x864 (1080p at 125%) | flyout | closed | 1536 x 822 (95%) |
| 1536x864 | flyout | bottom, 35% | 1536 x 520 |
| 1366x768 | flyout | closed | 1366 x 726 (95%) |
| 1366x768 | flyout | bottom, capped at 35% | 1366 x 457 |

**Focus mode** (Ctrl+Shift+F) hides every unpinned panel, for reviewing
and presenting (layout B). F11 is full screen.

**Structure board.** A new project opens on its structure: functions as
columns, locations as chips, typed in place. "Add page" under a function
opens the Schematic canvas. Afterwards structure is reached through the
breadcrumb and the outline filter, not as a separate place.

**Rules document.** `rules` (or Project settings) replaces the canvas with
the project's rules as sentences with inline choices: "Devices are numbered
[per letter code ▾]". Esc returns.

**Page frame.** Every page is drawn and printed in a frame, chosen by a rule
("Pages use [A3 landscape, company frame ▾]"), never per page by hand. The
frame carries:
- a numbered column grid (ten columns on A3 by default, set by the frame),
  which is what references point at;
- a title block: project, page title, page n of m, the page's `=function`
  and `+location`, revision letter and date, drawn by and checked by.

References read `page.column` (`3.4`), with the structure prefix only when
it differs from the reader's page (`=A2/3.4`). The dot belongs to
references, so a page name never contains one: a page inserted where no
whole number fits takes the next free letter (`3a` between `3` and `4`,
then `3b`; the same 23 letters as generated sheets, §8), and a reference
to it reads `3a.4`. Renaming a page to a name with a dot is refused with
that reason. Between `3` and an existing `3a` no letter fits, so the
lettered run moves up one letter (`3a` becomes `3b`) and the new page takes
`3a`; references follow, since they point at pages. Frozen names never
move: if the run has a frozen page, the new page goes at the end of the run
and the status line says so ("inserted as 3d: 3a–3c are frozen").
Revision fields come from the project's revisions, which also drive
Changes mode and the pinned-flow freeze (§8). Owners: WS-03 draws the frame (`PageScene::frame`), WS-10 owns
the reference format, and the frame choice is a rule like any other.

**Saving.** There are no save prompts and no Save step to remember. A named
project is written into its own `.novo` file continuously, about half a
second after each change and on close, so reopening shows exactly what was
left. An untitled project closed with work in it is kept in
`%LOCALAPPDATA%\NovoElectric\Unsaved` and offered back (under Unsaved with
Recent, and reopened at start-up if it was the last one open); naming it
moves it to `<Documents>\NovoElectric\<name>.novo`. The
deliberate checkpoints are revisions (§7, §8), not Save. Ctrl+S
names an untitled project and otherwise only confirms Saved; Ctrl+Shift+S
saves a copy under a new name and keeps working in the original.

**Start-up.** The app opens the last project where it was left: same page
and zoom, nothing selected. There is no start screen to click through. With
no project open (first run, or after Ctrl+W) the canvas area shows New
project, Open, Open example and Recent; it is the only time the canvas is
empty. New projects are saved to `<Documents>\NovoElectric\<name>.novo`,
with Documents resolved through the known-folder lookup because company PCs
redirect it to OneDrive. Nothing is written outside the user's profile, and
nothing needs admin rights.

**Language.** Every command word, parameter and label has an English name
and per-language aliases, Dutch first (`voeding 400V 32A` and
`supply 400V 32A` both work; `alles accepteren` is `accept all`). Both
always match, whatever the UI language, so a colleague's habit still works.
Device tags, IEC letter codes, units and potential names (L1, PE) are never
translated.

## 4. Commands

Every command lives in these places, in this order of preference. Nothing
lives only in a menu, and nothing opens a modal dialog in the drawing loop.

1. **Direct manipulation** for the most frequent things: drag from a pin to
   connect; drag a symbol, a junction dot or a text to move it (a drag that
   starts on a text moves only that text); drop a contact on a coil to give
   it its device; drag a device onto a location in the outline.
2. **The command surface.** Ctrl+K opens it at the top; I and a tapped
   Space open it at the pointer, acting on what is under it. Empty, it
   lists every command grouped (Project, Draw, Edit, Navigate, View, Check,
   Export) with shortcuts and disabled reasons: this is the menu, and there
   is no menu bar. Its grammar:
   - a noun selects or jumps: `K1`, `12`, `X2:1`. Enter on an exact tag
     jumps to the device's main location, and every other location is
     listed below;
   - a verb with a noun acts: `no K1`, `dol 4kW x3`, `renumber page`;
   - a bare verb runs a command: `rules`, `export pdf`, `problems`.
   **Every grammar form is reachable by picking rows** (verb · object ·
   parameter chips) without knowing the syntax, with a live preview on the
   drawing. Typing is a shortcut, never the only way.
3. **The context menu.** On an object, that object's actions first, best
   guess first. On empty canvas, "repeat last tool" first.
4. **Keys** (below). Letter keys act only while the drawing has focus.
5. **The inspector** for properties of the selection.

**Pin or body.** Pins win over the symbol body within 6 screen pixels of a
pin, at every zoom. Hovering there shows the pin as the connect target (a
steel-blue ring and the connect cursor) before the press, so a drag that
starts there connects and a drag anywhere else on the symbol moves it.

**Selection.** A click selects a device. A double-click on its text selects
the text and edits it. Alt+click selects the deepest item under the
pointer. Clicking empty canvas clears the selection.

**Escape ladder.** Cancel the step in progress, then leave the tool, then
clear the selection.

### Keys

| Key | Action |
| --- | --- |
| Ctrl+K | Command surface at the top |
| Space tapped | Command surface at the pointer |
| Space held and dragged (over 4 px) | Pan; middle drag also pans |
| Ctrl+1 / Ctrl+2 | Schematic / Cabinet |
| Ctrl+3 / Ctrl+4 / Ctrl+5 | Terminals / Cables / Devices drawer |
| Ctrl+T | Table face of the Cabinet canvas |
| Ctrl+Shift+F / F11 | Focus mode / full screen |
| I | Place (command surface in place mode at the pointer) |
| 1 to 9 | Favourite symbols, only while the place query is empty |
| W | Connect, for chains (one connection is a drag from a pin); Shift+W switches Corners / Lines, also in the middle of a wire |
| R / X | Rotate / mirror while placing or on the selection |
| F2 | Edit the selected tag or text |
| Enter | Accept the highlighted proposal |
| Tab | Next proposal |
| G / Shift+G | Go to partner / every occurrence |
| Alt+Left / Alt+Right | Back / forward through jumps |
| F8 / Shift+F8 | Next / previous problem |
| Shift+P | Hide or show marks (counters stay) |
| Shift+O (held) | Show override marks |
| Page Up / Page Down | Previous / next page |
| Home | Zoom to fit (also double middle click) |
| Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) | Undo / redo |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy, cut, paste with the next free tags |
| Ctrl+Shift+V | Paste with the original tags |
| Ctrl+D | Duplicate with the next free tags |
| Ctrl+S / Ctrl+N / Ctrl+O | Name an untitled project (otherwise confirms Saved), new, open |
| Ctrl+Shift+S / Ctrl+W / Ctrl+Q | Save a copy, close project, quit |
| Delete | Delete the selection and its dangling connections |
| Esc | The Escape ladder |
| ? | Shortcut overlay |

A jump (G, an exact tag, F8, Alt+Left/Right, a drawer row, Tab to an
off-screen proposal) moves the view only when it has to. If the target is
already readable (2.5 mm text at 10 px or larger) and at least 48 px inside
the view, nothing moves. Otherwise the view centres on the target and zooms
in to readable size (a closer zoom the user already had is kept), unless
the target is too big to fit, when it zooms out just enough.

There are no keys for T-pieces, terminals, cables or potentials, because
the user does not place those. Corners are clicks in a wire being made,
never symbols. "Repeat last tool" is the first item
of the context menu on empty canvas and the first row of an empty command
surface; it has no key, so Enter always means accept.

**Wiring modes.** The Connect tool has two modes, named for the two ways
CAD programs let you wire. Both make the same thing: a connection with a
stored route (corners here are the bend points of a route, never pieces
the user places, §8), so a project drawn in one mode edits the same in the
other.

- **Corners** (the default, EPLAN style): click where the wire should turn
  and the lines between your clicks connect by themselves, with at most two
  bends per leg. Each leg's elbow runs first along the axis the pointer has
  moved furthest from the last point; the preview shows it before the click
  and R flips it (R is already rotate while placing, so no new key). Click
  a pin, a junction or a wire to finish.
- **Lines** (AutoCAD style): you draw the lines. Each click commits one
  straight segment from the previous point, locked to the axis the pointer
  is nearer to (a diagonal pointer snaps to the nearer axis), and nothing is
  routed for you. Click a pin, a junction or a wire to finish; Enter or a
  double click finishes on the last point when it is on a pin or a wire.

**A drag from a pin is not a mode.** In any tool and either mode, pressing
on a pin and moving more than 4 px (the same threshold as pan) is the quick
connection: release on a pin or a wire and the router picks the whole
route; release on nothing and it cancels, as Esc does. A press that moves
less than 4 px is a click, so a shaky click never becomes a line. The
connection budget of 1 in §6 is the drag; a wire with N corners costs N + 2
clicks in either mode.

In both modes, Backspace takes back the last corner or segment and Esc
cancels the whole wire in progress (then the rest of the Escape ladder). A
wire that ends on another wire of the same potential makes a T-junction; on
a wire of a different declared potential it is refused with the reason. A
route never passes through another conductor's pin or junction. Lines are
orthogonal only (§12 for diagonals).

**The toggle.** Shift+W switches mode, from anywhere on the drawing and
also in the middle of a wire (the points already placed stay and the next
click follows the new mode). W is the wire, Shift+W is how wires are
drawn, the same pairing as G and Shift+G. A small two-segment chip
`Corners | Lines` sits right of the Connect icon in the tool dock, always
visible and never behind a hover. It is quiet (faint text) while another
tool is active, and clicking either segment selects Connect in that mode,
so tool and mode change in one action. The selected segment is the usual
lift, not copper, which stays on the active tool's underline. The Connect
icon itself carries the mode (an L for Corners, a straight segment for
Lines). The tooltip reads "Connect · W · Shift+W switches". The mode is
also named in the dock hint ("Lines: click a pin · click each corner · click a pin ·
Backspace undo · Shift+W corners"). Compact dock keeps the mode word in
the hint. A held Shift+W does not flip back and forth: key repeat is
ignored. The command surface lists `Wiring mode: corners` and `Wiring
mode: lines` under Draw (command `wire.mode`, typed "mode" or "lines").
The mode is a Preference of the user, not of the project, so two people
on one project each keep their own. Switching never changes a wire that
already exists.

**Lines never overlap.** The rule holds for every way a wire is made,
moved or edited, in both modes, and is enforced by the editor engine, not
the screen:

- two wires on a page never share any length; they may cross (a crossing
  without a connection gets no dot, §2) and may meet at a pin or a
  junction;
- two wires of one conductor that would run over each other from a shared
  pin diverge at a derived junction; the stretch is stored and drawn once;
- in Corners, a new or edited route that would lie over another conductor
  is nudged one grid step sideways, up to three steps, keeping its ends on
  their pins; if that cannot clear it, the gesture is refused;
- in Lines, a segment that would lie over another conductor is refused
  before it is placed, never nudged, because the line is exactly what the
  user drew. In Corners a clicked corner that lies on another conductor is
  refused the same way. The refusal is live while the pointer moves, not
  only on click, so there is never a dead click, and never colour alone
  (the transient mark in §5): not-allowed cursor, the clashing segment
  tinted with the Error colour, a dashed outline of the refused corner or
  line with a cross, the preview kept up to the last legal point, and the
  dock hint in Error text ("Overlaps -W3 L1"). Nothing opens a dialog;
- the reason is returned as data (the other conductor and the stretch), not
  only drawn as text, so the dock, the tester and scripts read the same
  answer;
- moving a symbol is never refused: the check re-runs on its wires as part
  of the same undo step, a wire that would overlap is nudged where it can
  be, and otherwise the overlap is accepted and shows in Issues;
- a file that arrives with overlaps (imported, older, or from that move) is
  not rewritten on load; novo-checks reports each as a warning (`wire.overlap`)
  that marks both segments, with the nudge as its fix. It is a warning, not
  an error: connectivity lives in the model, not in where the lines are;
- merging two wires of one conductor is one undo step. The surviving wire
  keeps its number and text and the other's is released; if both carry
  different explicit numbers the merge is refused with the reason.

A hand-drawn route (Lines, or Corners with clicked corners) is kept by
`tidy`; selecting it and choosing `tidy` re-routes it on purpose. Moving a
symbol stretches routed wires as before and stretches hand-drawn ones by
moving only their end segments, so every corner the user placed stays
where it was put.

Acceptance, for testers and reviewers, run in both modes unless marked:

1. W, drag pin to pin: a routed wire, no overlap (identical in both modes).
   Releasing a drag on nothing cancels. A press under 4 px is a click.
2. Shift+W: the chip and the hint change mode; Shift+W again: back. Holding
   Shift+W does not flip repeatedly. Shift+W in the middle of a wire keeps
   the points placed and the next click follows the new mode.
3. Corners, EPLAN style: click pin, click two corners, click pin: a wire
   through those corners, every segment horizontal or vertical; R flips an
   elbow, shown in the preview before the click.
4. Lines, AutoCAD style: click pin, click two corners, click pin: the same
   wire, with no segment the user did not click.
5. Lines: a segment over another conductor is refused live, with the
   cursor, tint, outline and reason in the hint, and nothing changes.
   Corners: a clicked corner over another conductor is refused the same
   way; a router leg is nudged clear or refused.
6. Two wires of one conductor from a shared pin store the shared stretch
   once, with a junction where they diverge; merging keeps the surviving
   number and text and refuses on two different explicit numbers.
7. Ending on a wire of the same potential makes a T-junction; ending on a
   different declared potential is refused with the reason.
8. Esc mid-wire cancels the whole wire; Backspace removes one corner or
   segment; undo removes one whole wire.
9. Moving a symbol never fails; an overlap it cannot avoid shows in Issues
   and marks both segments. A file with existing overlaps opens unchanged
   and shows them in Issues.
10. Quit and reopen: the mode is the one last used, read from the user's
    Preferences and not from the project file.

Keys match the logical character, not the physical position, so `?`,
Ctrl+Z and Ctrl+Y work on Dutch, German (QWERTZ) and US layouts alike.
While the drawing has focus it consumes Tab as "next proposal" instead of
letting focus move into the inspector; inside a focused field, Tab moves
between fields as usual.

## 5. Marks on the drawing

| Mark | Look | Text |
| --- | --- | --- |
| Selection | Copper, 2 px | none |
| Hover | Steel blue | none |
| Proposal (terminals, cable, part) | Dashed steel blue at 55%, 1 px; the highlighted one dashed at full strength, 2 px. Proposals are never solid, so none can be mistaken for a drawn wire | "↵ accept · Tab next" on the highlighted one only |
| Warning | Small amber triangle, top right of the object, screen sized | on hover |
| Error | Small red octagon, same place | on hover |
| Refused wire (transient, never stored) | The clashing segment tinted Error, a dashed Error outline of the refused corner or line with a cross, the Error icon in the dock hint | "Overlaps -W3 L1" in the dock hint; the not-allowed cursor |
| Override | Hollow grey diamond, top left, while Shift+O is held | the inspector shows "override" and a reset arrow |
| Change (Changes mode only) | Added green, modified amber, removed faint red dashed ghost | listed in the outline |

- Marks keep their screen size at every zoom and cluster into a count per
  area when zoomed out. Shift+P hides them; counters remain.
- Copper is never a mark, so selection is always unambiguous.
- Proposal, problem and override differ in colour, line style and
  position at once, so they stay apart for colour-blind users.
- Change marks appear only in Changes mode, so they never compete with
  proposals and problems.
- **Which proposal Enter takes.** Exactly one proposal is highlighted, and
  it carries the hint, so Enter never acts on something unmarked. The
  highlight follows the pointer to the nearest proposal on the page; Tab
  moves it to the next one in page order and holds it there until the
  pointer travels more than 24 px. Delete on the highlighted proposal
  declines it.

## 6. Journeys and budgets

An action is one click, one key press, one chord or one drag; typed words
are listed separately. A budget is a ceiling for the default path; anything
slower fails review.

### From a blank project to a finished panel drawing

A supply and three pumps:

| Step | What the user does | Actions | Typed |
| --- | --- | --- | --- |
| 1. Start | Ctrl+N, name, Enter. The structure board opens with the template's `=A1 Supply`, `=A2 Loads`, `+CAB1`, `+FIELD`; "Add page" under `=A1`. | 4 | name |
| 2. Supply † | `supply 400V 32A`, Enter, click. Incoming terminals, main switch and PE bar, with "3~ 400 V + N + PE" declared at the source. | 4 | `supply 400V 32A` |
| 3. Three feeders † | `dol 4kW x3`, Enter, click. Three tagged feeders with parts sized. | 3 | `dol 4kW x3` |
| 4. Field devices † | Ctrl+5, select the three motors, set `+FIELD` once in the inspector. | 4 | `+FIELD` |
| 5. Terminals and cables † | `accept all` shows "9 terminals on -X1, 3 cables -W1..3"; Enter. | 3 | `accept all` |
| 6. Control by hand | Place two push buttons and a lamp, connect five pins. | 14 | `pb`, `lamp` |
| 7. Parts left over † | Issues shows "2 devices without a part"; select both in Devices, pick the part once. | 4 | part search |
| 8. Cabinet † | Ctrl+2, `place all`, review the preview, Enter. Rails that would overflow are warned about first. | 3 | `place all` |
| 9. Output | `export pdf`, Enter. Export is never blocked; its summary lists remaining warnings. | 2 | `export pdf` |
| **Total** | | **41** | |

† Needs something §10 puts after Gate 1: blocks (steps 2 and 3), the
Devices drawer (4), terminal proposals in the editor (5), the parts
database (7) or the Cabinet canvas (8). Review Gate 1 work against the Gate
1 benchmark below, not against these steps. In Gate 1 the supply is
declared by hand (3 actions, see supporting journeys).

### The first five minutes

Someone who has never seen NovoElectric, with no manual, gets to a
connected circuit and finds a command without a menu bar:

| Step | What they see and do | Actions | Typed |
| --- | --- | --- | --- |
| 1. Launch | The canvas area shows New project, Open, Open example and Recent. Click New project, type a name, Enter. | 2 | name |
| 2. Structure | The board's hint reads "Add a page under a function". Click Add page. | 1 | |
| 3. First symbol | The empty page reads "Press I or Space to place a symbol · ? for keys". I, type `contactor`, Enter, click. | 3 | `contactor` |
| 4. Second symbol | The dock hint reads "Click to place another · Esc stop"; click once more (tools are sticky). | 1 | |
| 5. Connect | Hovering a pin shows the connect ring and the dock hint "Drag from a pin to connect"; drag. | 1 | |
| 6. Keys | ? shows the shortcut overlay; ? or Esc closes it. | 2 | |
| 7. A command | Ctrl+K: the empty surface lists every command by group. Pick Export PDF. | 2 | |
| **Total** | | **12** | |

Every hint names the next action and its key, sits next to where the eye
already is (the page, the dock, the hovered pin), and stops showing once it
has been followed three times. "Show hints again" is in the command surface.

### Gate 1 benchmark

One direct-on-line starter over two pages, on the hand path: **54 actions**
(new project and pages 6, supply declaration 3, nine placements 27, 17
connections by drag, one proposal accept). With a block: 11.

### Supporting journeys

| Task | Actions |
| --- | --- |
| Place a further device of the same kind | 1 click (tools are sticky) |
| Place a contact of `-K1` | 3 (I, `no K1`, Enter, click) |
| Connect two pins | 1 drag |
| T-junction onto a connection | 1 drag onto the line |
| Rename a device | 2 (F2, Enter) plus the tag; renaming a coil renames its contacts |
| Renumber a page | 3, with a preview, one undo step |
| Move one text | 1 drag |
| See everything a device switches | 0 (select it: the page lights up, the rest is in Relations) |
| Go to partner / back | 1 (G) / 1 (Alt+Left) |
| Every occurrence of a device | 1 (Shift+G) |
| Find a device by exact tag | 2 (Ctrl+K, Enter) plus the tag |
| Declare a supply | 3 (select the incoming terminals, choose the system in the inspector) |
| Override one conductor's potential | 2 |
| Accept one proposal / every proposal | 1 (Enter) / 3 (`accept all`, preview, Enter) |
| Same property on 50 devices | 3 plus the value (select, inspector search, set) |
| Rename 400 terminals by pattern | 3 plus the pattern (Ctrl+3, find and replace, Enter) |
| Work through every problem | F8 per problem |
| Change a project rule | 3 (`rules`, pick the chip, choose) |

**Numbering:** per letter code (`-K1`, `-K2`) or per page within the product
aspect (`-K301` for the first contactor on page 3). Typing `-K5` keeps the
page's function and location. A duplicate is refused while typing.

**Letter codes** follow IEC 81346-2 by what a device is for, and come from
the symbol or part, not from the user: a miniature circuit breaker that
protects a circuit is F (`mcb`), a motor protective circuit breaker or a
circuit breaker that switches a feeder is Q (`mpcb`), an overload relay is
F. Contactors and relays are K by default: IEC 81346-2:2019 codes a
power contactor Q, but K is what most existing projects and panel builders
use, and a default that renumbers every contactor would cost more than it
teaches. Because companies differ here, each class has a rule sentence
("Circuit breakers are coded [by purpose ▾ | always Q | always F]",
"Contactors are coded [K ▾ | Q]"), and a typed tag always wins. The
contactor rule recodes contactor coils and main contacts only; control
relays and their auxiliary contacts stay K, as 81346-2 codes them.

Letters follow the 2019 edition, which codes by purpose rather than by
component type: inductors, filters, diodes and rectifiers are R
(restricting a flow), transistors are K and thyristors and triacs Q
(switching signals or energy), and the older V for semiconductors and L
for inductors (DIN 40719, IEC 60750) are not used. A PLC
and its I/O modules are K (processing signals), not A: A is for objects
with several purposes, and K is also what the 2019 edition lists for a
PLC. Panel builders used to the older V, L and A will see these change.
The one table lives in `novo_symbols::letter_code`.

**Undo:** Ctrl+Z undoes the last gesture in every canvas, drawer and field,
and brings the affected page into view. Text fields keep their own Ctrl+Z
while focused. The undo history lives for the session; crash recovery
restores the project, not the undo stack.

## 7. Rules and blocks

**Rules** are written once per project as sentences: numbering scheme,
terminal joining ("terminals with the same name are one terminal, joined
with jumpers"), text style and position, wire numbering, conductor colours,
name recognition for PE, N and PEN. Changing a rule updates the project
live. An instance that differs from a rule is an override: marked, counted
and resettable.

**The part works the same way for each device.** Its data (rating, size,
look) is the default, and any per-device value is an explicit override,
marked and resettable to the part (WS-07). Rules decide how things are
drawn; parts decide what they are. Together they are why 400 identical
terminals stay identical, and why "why does this one differ?" always has an
answer in the inspector.

**Blocks** are parametric circuits. Placing one, changing a parameter and
applying a definition update are each one undo step. For an instance the
user has edited:
- layout edits survive as overrides (moved symbols and texts, re-routed
  connections);
- identity edits survive (a hand-changed tag is never undone by a renumber
  or update);
- parameter values survive;
- structural edits (adding or removing a device or connection inside the
  block) detach the instance, and the inspector says why;
- an update previews what changes and which overrides stay.

**Where blocks live.** A definition lives in the project, so a `.novo` file
is always complete on its own. Libraries are sources to copy from: the
user's own `%APPDATA%\NovoElectric\Blocks` and, optionally, a company
folder the user points at (any readable path, no admin rights). A project
copy remembers its source and version, so a newer library version is
offered as an update, never applied silently.

**What a block substitutes.** Tags are allocated, not substituted: each
member keeps its letter code and gets the next free number from the
numbering rule (`dol 4kW x3` places three instances, not one with a count).
Property values (power, current) and texts that name a parameter
("{power} kW") are filled in. Parts are not parameters: after placement the
part proposals run on the members' sizing, automatic where one part fits.

### Revisions

A **revision** is a deliberate issue of the drawing set: index, date,
author and a one-line description ("B · 2026-10-15 · TV · Motor M3 added").
It is the checkpoint that saving is not (§3).

**Making one.** "Issue revision" is one command and one undo step. It
proposes the next index from the revision rule ("Revisions are indexed
[A, B, C, ... ▾]", skipping I and O; or 0, 1, 2, ...; or `01`, `02`), today's
date and the user's initials, and takes only the description. The model
records a `Revision` (index, date, author, description); storage keeps a
snapshot of the whole project under the revision's id beside the records,
so the `.novo` file stays one file and an older revision can be opened
read-only or compared. Snapshots are stored compressed, so twenty
revisions cost little more than a few copies of the project. Undo removes
the record and puts every page's freeze back as it was before the issue
(undoing B restores the A freezes). A snapshot whose revision is undone is
kept while the undo history can still redo it, and dropped only on open or
close, so undo, autosave and redo never bring back a revision without its
snapshot. Issuing with nothing changed since the last revision is allowed
(a reissue is a real event), and the command line says "no changes since
B".

**Pinned flow.** Issuing a revision freezes every schematic page with that
index (`FreezePage`, #119), so a `page.column` reference in the issued set
never moves. Issuing the next revision re-takes every freeze, so the new
set prints where things are now. The freeze carries the project's index,
which is what the issued set was; a page deleted after A is simply absent
from B's set, and Changes lists it as removed. Generated sheets are never
frozen; they reflow (§8). A plain export is not a checkpoint: it prints the current
state and freezes nothing. Issuing is the way to say "this set went out".

**Title block.** The project revision is the latest one. Each page shows
its own revision: the latest one in which that page changed, with its date
(an untouched page keeps "A" while the project is at "D"), worked out
from the change sets between revisions rather than stored. Frames with a
revision table list every revision: index, date, author, description.

**What changes compare.** Changes (Outline) and the change marks compare
two states, by default the latest revision and now; any two revisions can
be picked. The comparison is on the model, not on pixels. Records are
matched by id and each one is added, removed or changed (with the fields
that differ). A change lands on every page it shows on: a symbol, wire,
junction or text on its own page; a device, terminal or cable on every
page that draws it; a renamed or moved page on that page. Changes with no
page (parts, potentials, rules, structure) are listed under Project. On the
canvas, Changes mode shows the change marks (§5); removed objects are
drawn as ghosts from the older state, so the user sees what went. Pages
with changes are marked in Pages and in the PDF bookmarks.

Owners: WS-02 the `Revision` record and its commands, WS-04 the snapshot
table, WS-18 (`novo-revisions`) the index rule, issuing, the per-page
change set and the title block fields, WS-03 and WS-06 the marks and
Changes mode, WS-09 the PDF.

## 8. Decided alternatives

| Question | Options considered | Decision |
| --- | --- | --- |
| Pages | Hand-made; flowing; pinned flow | **Pinned flow:** automatic pagination, user page breaks never move, and pages freeze when a revision is issued so `page.column` references never move under a printed set |
| Generated pages under pinned flow | Freeze them too; append new sheets at the end; reflow | **Reflow.** Generated pages (BOM, terminal and cable diagrams, lists) sit in their own numbered sections after the schematic pages, so their growth never renumbers a schematic page. No reference points into them, so they reflow freely inside their section; the PDF bookmarks and the contents list name each sheet by what it shows (strip -X1, cable -W3), and a revision marks the generated sheets whose content changed; anything that ever links into a generated sheet points at it by name, never by `page.column`. A generated sheet's "n of m" counts within its own section, and a schematic title block's "n of m" counts schematic pages only, so a growing section never changes a frozen title block. A generated sheet is named by its section number plus a letter (`20a`, `20b`, ... `20z`, `20aa`, skipping `i`, `l` and `o` so `20l` never reads as `201`), never with a dot, so a sheet name can never be read as a `page.column` reference. Inserted schematic pages use the same letters (§3); generated sections are numbered after the schematic pages (the contents list alone opens the set as section `0`; it reflows like any other section and is not counted among schematic pages), and if a name ever clashes the schematic page keeps it and the sheet takes a `'` mark (`20b'`) |
| Routing | Manual; always automatic; route once and keep | **Route once and keep:** moving a symbol stretches its connections; `tidy` re-routes on request |
| Where a project starts | Page-first; structure-first; intent-first | **Structure-first**, with the empty page offering "type what you are building" |
| Window | A Workbench; B Focus; C Split | **A**, with B as Focus mode and C as the drawer docked right |
| Top-level places | Five lenses; two canvases plus drawers | **Two canvases (Schematic, Cabinet) and one drawer (Terminals, Cables, Devices)**; reports are commands |
| Menu | Menu bar; empty command surface | **No menu bar**; the empty command surface is the menu |
| Command line | Permanent; on demand | **On demand**, at the top or at the pointer |
| Tab | Orientation; next proposal | **Next proposal**; R and X orient |
| Facing pins at a distance | Connect on placement; propose; ignore | **Propose**, in the placement preview; touching pins connect |
| Command language | English only; translated words; aliases | **Aliases**: English and Dutch words both match; tags and IEC codes never translate |
| Start-up | Start screen; last project | **Last project**, where it was left; the empty canvas offers New, Open, Example and Recent |
| Saving | Save command and prompts; continuous | **Continuous**, into the project's own file; revisions are the checkpoints (see Checkpoints) |
| Checkpoints | Every export; revisions only; both | **Revisions only:** issuing a revision freezes the schematic pages and keeps a snapshot; a plain export prints the current state and checkpoints nothing, so Changes always compares against something with a name |
| Junction dots | Every meeting point; three or more conductors | **Three or more**; straight runs, corners and crossings get none |

## 9. What we leave behind

| Convention | Seen in | Why not |
| --- | --- | --- |
| The page as the source of truth | EPLAN, AutoCAD Electrical | Root cause of navigators, manual linking and update steps |
| A navigator per object type | EPLAN | One outline with a structure filter and one inspector |
| Separate editors for terminals and cables | EPLAN | They hide the schematic when you need it; the drawer keeps them beside it |
| Insert centre, symbol trees, ribbon | EPLAN, AutoCAD Electrical | Spatial hunting; the command surface finds by name |
| Frozen macros | Most CAD | Go stale on placement; blocks stay linked |
| Corner and T-piece symbols | EPLAN | Drawing artefacts of connectivity, not intent. The corners of a wire are bend points of its route, not objects |
| Wiring modes | One way; Auto plus a merged Draw; Corners and Lines as two modes | **Two modes, Corners (default) and Lines.** They match the two ways Tadjiro knows from EPLAN and AutoCAD, and they differ in a real way: Corners lets the router join the points, Lines routes nothing and refuses an overlap instead of nudging it. A drag from pin to pin stays the cheapest connection in both |
| Linking cross references by hand | EPLAN | References follow ownership |
| Assigning potentials wire by wire | EPLAN | The source declares it once |
| Save prompts and "save before closing?" | Most desktop software | Work is never lost and never asked about |
| Autoconnect at any distance, without warning | EPLAN | Connections the user never saw; facing pins are proposed in the preview |
| Start screens and tips of the day | Most CAD | A click between the user and their project; hints sit where the next action is. The empty-canvas New/Open/Recent view is not one: it appears only when no project is open, never in front of one |
| Tabbed property dialogs | Windows CAD | Relations first, then search |
| Update, build and check runs | EPLAN | Everything is live; problems are marked in place |
| File/Edit/View menu bar | Windows applications | A second, worse index of commands |
| Permanent command line and jargon commands | AutoCAD | Splits attention; our surface opens where you look and every form is pickable |
| Status bar | Most CAD | Hint in the dock, coordinates in the HUD, checks in the pill |
| Page tab strip | Most CAD | Fails at 500 pages; Recent does its job |
| Infinite canvas | Figma, Miro | IEC documentation is printed and referenced by page and column |
| Floating panels everywhere | Figma-style tools | Weak spatial memory; only the dock and inspector float |
| New-project wizards | Most CAD | A name and Enter; the rest is defaults and rules |

## 10. What the model needs, and what Gate 1 needs

Model additions for WS-02:
- a feed on a terminal or pin, so a source is not a fake symbol;
- a `PotentialSystem` record grouping L1, L2, L3, N and PE with the line
  voltage;
- `Wire.potential: Option<PotentialId>` for a single-conductor override;
- a stored dismissal for declined proposals (WS-11 uses a wire property
  until then);
- `BlockDefinition` and `BlockInstance` with member back-references;
- an enclosure flag on locations, so terminal proposals pick the cabinet
  side reliably;
- a `Revision` record (index, date, author, description) and an
  "issue revision" command that freezes the schematic pages (§7).

| In Gate 1 | Later |
| --- | --- |
| Schematic canvas, drag to connect, routes stored once | Symbol-avoiding routing, `tidy` |
| References by ownership, G, Shift+G | Linked blocks (parametric placement only for now) |
| Potentials from a source | Live restyling from rules |
| Command surface with nouns, verbs and pickable rows | Cabinet canvas |
| Numbering rule, F2 rename, renumber | Drawer tabs beyond Terminals |
| Problem marks, Issues list, F8 | Terminal proposals in the editor (WS-11 derives them already) |
| Page frame with column grid, `page.column` references | Dutch command aliases |
| Start-up, first-run hints, `?` overlay | Revision fields filled from revisions |
| | Part proposals (needs the WS-07 parts database) |
| | Revision clouds in the PDF |

## 11. Principles and review checklist

Principles:
1. No modal dialog in the drawing loop.
2. Defaults first: every new object gets a valid tag, number and position.
3. State intent; the program does the bookkeeping.
4. Rules over instances; overrides are visible and resettable.
5. The drawing never moves because of chrome.
6. Every command is in the command surface and pickable without syntax.
7. Problems and proposals are marked where they are, before they become
   problems.
8. One gesture, one undo step; no save prompts.
9. Plain names; no family of navigators.
10. Live everywhere: a 50-page project opens in under 2 seconds and
    generated pages update within a second.

Checklist for every UI pull request:
- Does it add a dialog, a mode, a panel or a key where intent and a rule
  would do?
- Does it add an object the user must place or rotate that the program
  could derive?
- Is every new command in the command surface, pickable, with its key?
- Does anything move the drawing when a panel opens?
- Does Escape follow the ladder, and is each gesture one undo step?
- Do letter keys act only while the drawing has focus?
- Does a new look come from a rule, with overrides marked?
- Does the journey stay within its budget?

## 12. Open questions

- Does the dock moving aside during gestures feel calm in a real build?
- Diagonal lines in Lines mode (Shift to release the axis lock, 45 degrees).
  They add a second geometry to every overlap, junction and hit-test rule
  and to the PDF; left out until someone draws a sheet that needs one.
- A light theme for review in bright rooms. Dark stays the default; tokens
  are named by role, so a light set can be added without touching screens.
- How much of the structure board is a form and how much is drawing?
- The inspector flips sides when the selection is under it, which is the
  one place the window moves under the eye. Would a fixed right-hand
  inspector that turns compact or translucent over the selection be
  calmer? Decide in a real build.

## Appendix: EPLAN symptoms and where they are answered

| Symptom in EPLAN | Answered by |
| --- | --- |
| Hundreds of panels all called "navigator" | §3 outline with structure filter, §9 |
| Single texts hard to move, rotate, change | §4 drag a text; F2; R |
| Texts and 400 identical terminals look different | §7 rules and overrides |
| Unclear when terminals connect | §2 boundary proposals; terminals show through or end |
| Same-named clamps should join with 1-in/1-out terminals | §7 terminal joining rule |
| Layout generator manual, no heads-up | §6 step 8, warnings before placement |
| Potential assignment sluggish | §2 potentials from sources |
| Numbering painful | §6 numbering, §7 rules |
| Long build times | Principle 10 |
| Cryptic properties screen | §3 inspector, Relations first |
| T-pieces cannot be rotated; scattered shortcuts | §2 no connection pieces |
| Cross references hard to link | §2 references follow ownership |
| No reliable shortcut to all references | Shift+G |
| Autoconnect surprises | §2 facing pins are proposed in the preview |
| Search cannot find a device by exact name | §4 exact tag jumps, all locations listed |
| Selecting a relay does not highlight what it switches | §6 select to light up, Relations |
| Insert centre hunt for common parts | §4 command surface, favourites, blocks |
