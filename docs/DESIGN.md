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
| Junction dots and T-junctions | Where stored routes meet | Automatic; a dot can be dragged |
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
- **Saved:** autosave state.

**Outline (left, 248 px).** Pinned above 1700 px wide, a flyout below;
pinned from its pin button or the "Pin outline" command. Three modes:
- **Pages:** Recent, then All pages. The filter takes text and structure
  (`=A1`, `+CAB1`, `motor`): the 81346 tree filters pages rather than
  owning them, because one page can hold several functions.
- **Issues:** problems and proposals in one sortable list with filter
  chips, worked through with F8 and Shift+F8.
- **Changes:** what changed since the last revision or export, by page.

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
| W | Connect, for chains (one connection is a drag from a pin) |
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
| Ctrl+S / Ctrl+N / Ctrl+O | Save, new, open |
| Ctrl+Shift+S / Ctrl+W / Ctrl+Q | Save as, close project, quit |
| Delete | Delete the selection and its dangling connections |
| Esc | The Escape ladder |
| ? | Shortcut overlay |

There are no keys for corners, T-pieces, terminals, cables or potentials,
because the user does not place those. "Repeat last tool" is the first item
of the context menu on empty canvas and the first row of an empty command
surface.

## 5. Marks on the drawing

| Mark | Look | Text |
| --- | --- | --- |
| Selection | Copper, 2 px | none |
| Hover | Steel blue | none |
| Proposal (terminals, cable, part) | Dashed steel blue at 55% | "↵ accept · Tab next" on the nearest one only |
| Warning | Small amber triangle, top right of the object, screen sized | on hover |
| Error | Small red octagon, same place | on hover |
| Override | Hollow grey diamond, top left, while Shift+O is held | the inspector shows "override" and a reset arrow |
| Change (Changes mode only) | Added green, modified amber, removed faint red dashed ghost | listed in the outline |

- Marks keep their screen size at every zoom and cluster into a count per
  area when zoomed out. Shift+P hides them; counters remain.
- Copper is never a mark, so selection is always unambiguous.
- Proposal, problem and override differ in colour, line style and
  position at once, so they stay apart for colour-blind users.
- Change marks appear only in Changes mode, so they never compete with
  proposals and problems.

## 6. Journeys and budgets

An action is one click, one key press, one chord or one drag; typed words
are listed separately. A budget is a ceiling for the default path; anything
slower fails review.

### From a blank project to a finished panel drawing

A supply and three pumps:

| Step | What the user does | Actions | Typed |
| --- | --- | --- | --- |
| 1. Start | Ctrl+N, name, Enter. The structure board opens with the template's `=A1 Supply`, `=A2 Loads`, `+CAB1`, `+FIELD`; "Add page" under `=A1`. | 4 | name |
| 2. Supply | `supply 400V 32A`, Enter, click. Incoming terminals, main switch and PE bar, with "3~ 400 V + N + PE" declared at the source. | 4 | `supply 400V 32A` |
| 3. Three feeders | `dol 4kW x3`, Enter, click. Three tagged feeders with parts sized. | 3 | `dol 4kW x3` |
| 4. Field devices | Ctrl+5, select the three motors, set `+FIELD` once in the inspector. | 4 | `+FIELD` |
| 5. Terminals and cables | `accept all` shows "9 terminals on -X1, 3 cables -W1..3"; Enter. | 3 | `accept all` |
| 6. Control by hand | Place two push buttons and a lamp, connect five pins. | 14 | `pb`, `lamp` |
| 7. Parts left over | Issues shows "2 devices without a part"; select both in Devices, pick the part once. | 4 | part search |
| 8. Cabinet | Ctrl+2, `place all`, review the preview, Enter. Rails that would overflow are warned about first. | 3 | `place all` |
| 9. Output | `export pdf`, Enter. Export is never blocked; its summary lists remaining warnings. | 2 | `export pdf` |
| **Total** | | **41** | |

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

## 8. Decided alternatives

| Question | Options considered | Decision |
| --- | --- | --- |
| Pages | Hand-made; flowing; pinned flow | **Pinned flow:** automatic pagination, user page breaks never move, and pages freeze after a revision or export so `page.column` references never move under a printed set |
| Routing | Manual; always automatic; route once and keep | **Route once and keep:** moving a symbol stretches its connections; `tidy` re-routes on request |
| Where a project starts | Page-first; structure-first; intent-first | **Structure-first**, with the empty page offering "type what you are building" |
| Window | A Workbench; B Focus; C Split | **A**, with B as Focus mode and C as the drawer docked right |
| Top-level places | Five lenses; two canvases plus drawers | **Two canvases (Schematic, Cabinet) and one drawer (Terminals, Cables, Devices)**; reports are commands |
| Menu | Menu bar; empty command surface | **No menu bar**; the empty command surface is the menu |
| Command line | Permanent; on demand | **On demand**, at the top or at the pointer |
| Tab | Orientation; next proposal | **Next proposal**; R and X orient |

## 9. What we leave behind

| Convention | Seen in | Why not |
| --- | --- | --- |
| The page as the source of truth | EPLAN, AutoCAD Electrical | Root cause of navigators, manual linking and update steps |
| A navigator per object type | EPLAN | One outline with a structure filter and one inspector |
| Separate editors for terminals and cables | EPLAN | They hide the schematic when you need it; the drawer keeps them beside it |
| Insert centre, symbol trees, ribbon | EPLAN, AutoCAD Electrical | Spatial hunting; the command surface finds by name |
| Frozen macros | Most CAD | Go stale on placement; blocks stay linked |
| Corner and T-piece symbols | EPLAN | Drawing artefacts of connectivity, not intent |
| Linking cross references by hand | EPLAN | References follow ownership |
| Assigning potentials wire by wire | EPLAN | The source declares it once |
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
  side reliably.

| In Gate 1 | Later |
| --- | --- |
| Schematic canvas, drag to connect, routes stored once | Symbol-avoiding routing, `tidy` |
| References by ownership, G, Shift+G | Linked blocks (parametric placement only for now) |
| Potentials from a source | Live restyling from rules |
| Command surface with nouns, verbs and pickable rows | Cabinet canvas |
| Numbering rule, F2 rename, renumber | Drawer tabs beyond Terminals |
| Problem marks, Issues list, F8 | Terminal proposals in the editor (WS-11 derives them already) |
| | Part proposals (needs the WS-07 parts database) |

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
- A light theme for review in bright rooms. Dark stays the default; tokens
  are named by role, so a light set can be added without touching screens.
- How much of the structure board is a form and how much is drawing?
- Should blocks live in the project, a shared company library, or both?

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
| Search cannot find a device by exact name | §4 exact tag jumps, all locations listed |
| Selecting a relay does not highlight what it switches | §6 select to light up, Relations |
| Insert centre hunt for common parts | §4 command surface, favourites, blocks |
