# Workflow specs

How NovoElectric should feel to an electrical designer who comes from EPLAN:
fewer clicks, no dialogs in the drawing loop, and every action on the
keyboard. This file is owned by the workflow thread. Each journey below has a
click and keystroke budget and acceptance criteria that UI pull requests
(WS-01, WS-06, WS-08, WS-10, WS-11, WS-12) are reviewed against.

The EPLAN numbers are estimates from typical default EPLAN setups, not
measurements; they are there to show the size of the gap. The journeys are
shaped by the EPLAN pain points Tadjiro listed, summarised in the next
section.

## EPLAN pain points this answers

Tadjiro's list (2026-10-14), each mapped to where this spec fixes it and the
workstream that builds it.

| Pain point in EPLAN | What NovoElectric does instead | Where | Owner |
| --- | --- | --- | --- |
| Hundreds of panels all called "navigator" | One project tree with filters; every panel has a plain, distinct name | Principle 11 | WS-01, layout thread |
| Single texts are hard to move, rotate and change | Click a text to grab it alone; drag, R, F2 | J11 | WS-06 |
| Text and the same terminals look different everywhere | Styles come from one definition per kind; 400 identical terminals look identical | Principle 12 | WS-06, WS-11 |
| Unclear when terminals connect and when not | Connection is drawn visibly; one project rule decides how same-named terminals join | J5 | WS-11 |
| Same-named clamps (`-X2:1`) should join through 1-in/1-out terminals with a connector | Project setting "Join terminals with the same name", applied live | J5 | WS-11 |
| Layout generator is manual and gives no heads-up | Unplaced devices listed live, auto-place with a preview, warnings before it does not fit | J13 | WS-15 |
| Potential assignment is sluggish | Select L1, L2, L3, right click, pick the suggested "3~ 400 V"; PE, N and PEN inferred | J10 | WS-06 |
| Numbering is painful | One numbering scheme per project, live on placement, preview before renumber | J4 | WS-06, WS-08 |
| Long project build times | No build step; everything is live within budgets | Principle 15 | all |
| Everything goes through a cryptic properties screen | Searchable properties panel, common fields first | J12 | WS-07 |
| T-pieces cannot be rotated with Tab; corner and T shortcuts are scattered | One key for connection pieces, Tab cycles every orientation | J3 | WS-06 |
| Cross references cannot simply be linked by right click | Right click, "Link cross reference", click the partner | J6 | WS-10 |
| No reliable shortcut to all references of a relay | Shift+G lists every reference of the selected device | J6 | WS-10 |
| Search cannot find a component's location by its exact name | An exact tag returns every location (page and column) first | J8 | WS-08 |
| Selecting a relay does not highlight what it switches | Selecting a coil highlights its contacts on the page and lists the rest | J6 | WS-10 |
| Common components need a trip to the Insert Center | Insert by mnemonic (`no`, `nc`, `coil`) or favourite keys 1 to 9 | J2 | WS-08 |

## How budgets are counted

- **Action**: one mouse click, one key press, or one key chord (Ctrl+K is one
  action). A drag counts as one action.
- **Typed**: characters of real content the user types (a tag, a search
  word). Counted separately because search shortens with use; the budget
  gives the typical number.
- Budgets assume the app is open with a project and page in view.
- A budget is a ceiling for the default path. Faster expert paths are
  welcome; slower ones fail review.

## Principles

These apply to every journey and every UI pull request.

1. **No modal dialog in the drawing loop.** Placing, wiring, tagging,
   terminals and cables never open a dialog. Details go in the properties
   panel, which is non-modal and edits the selection.
2. **Sensible defaults first, ask never.** Every new object gets a valid tag,
   number and position without a question. The user corrects afterwards, in
   place.
3. **Sticky tools.** A tool stays active after use so the next placement or
   wire is one click. Escape leaves it.
4. **One Escape ladder.** Escape cancels the step in progress, then leaves the
   tool, then clears the selection. It works wherever the keyboard focus is
   in the drawing area, not only while the pointer hovers it.
5. **Everything in the palette.** Every command, page, device and symbol is
   reachable from Ctrl+K by typing a few letters. Menus are a map of the
   palette, never the only door.
6. **One action, one undo step.** Ctrl+Z undoes exactly what the user did in
   one gesture, even when it changed many objects (a renumber of 200 tags is
   one step). Undo restores the view and selection of that step and never
   asks a question.
7. **No save prompts.** Autosave (WS-04) means closing never asks "save
   changes?"; Ctrl+S is a reassurance, not a duty.
8. **The status bar says what happens next.** While a tool is active the
   status bar names the next click ("Click the second pin, Esc to stop") and
   the keys that modify it (R rotate, X mirror).
9. **Live, not "update".** Cross references, terminal and cable pages, reports
   and checks are always current. No journey ends with a refresh step.
10. **Single-letter keys only on the canvas.** Plain letter shortcuts work when
    the drawing has focus and never steal typing from a text field.
11. **Plain names, no navigators.** Every panel and menu says what it holds
    ("Devices", "Terminal strips", "Cables"), never a generic name shared by
    many windows. One project tree with filters replaces a family of
    separate navigators.
12. **Same kind, same look.** The look of texts, terminals, wires and
    symbols comes from one style per kind, set once for the project.
    Changing the style changes every instance; an instance differs only when
    the user overrides it on purpose, and the override is visible in the
    properties panel.
13. **Selection shows relations.** Selecting a device highlights everything
    linked to it on the page (a coil's contacts, a terminal's other side, a
    cable's cores) and the status bar lists the off-page ones.
14. **The right click knows the selection.** The context menu offers the
    commands that fit what is selected, best guess first (assign "3~ 400 V"
    to three phase wires, link a cross reference to a lone contact).
15. **No build step.** Opening a 50-page project takes under 2 seconds,
    edits show in the same frame, and generated pages and reports update
    within a second (WS-13). Nothing has to be "generated" or "updated" by
    hand.

## Shortcut map

One consistent scheme. Letters are tools (active while the canvas has focus);
Ctrl chords are commands that work anywhere. Pull requests that add actions
pick from this table or extend it here first.

| Key | Action | Notes |
| --- | --- | --- |
| Ctrl+K | Command palette | Commands, pages, devices and symbols in one list |
| Ctrl+N / Ctrl+O / Ctrl+S | New, open, save project | |
| Ctrl+Shift+S | Save as | |
| Ctrl+W | Close project | Never asks to save (autosave) |
| Ctrl+Q | Quit | |
| Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) | Undo / redo | |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy, cut, smart paste | Paste gives the next free tags |
| Ctrl+Shift+V | Paste with the original tags | For moving circuits between projects |
| Ctrl+D | Smart duplicate | Copy next to the original, tags incremented |
| Ctrl+A | Select all on page | |
| Delete | Delete selection | Wires attached to deleted pins are removed too |
| F2 | Edit tag of the selected device in place | Enter commits, Esc cancels |
| Enter | Repeat the last tool | When nothing is in progress |
| Tab | Next orientation | While placing a connection piece, symbol or text: cycles through every orientation |
| I | Insert symbol | Opens symbol search at the cursor |
| W | Wire | Pin to pin, orthogonal |
| T | Terminals | Click or drag across wires |
| C | Cable | Drag across wires |
| P | Potential / interruption point | |
| J | Connection piece (corner, T, cross) | Tab cycles all 9 shapes and orientations in a fixed order |
| M | Move selection | Also by dragging |
| R | Rotate 90° | While placing, moving or on the selection, texts included |
| X | Mirror | While placing, moving or on the selection |
| G | Go to partner | Coil to contact, contact to coil, arrow to arrow |
| Shift+G | All references | Lists every place the selected device occurs; Enter jumps |
| 1 to 9 | Favourite symbols | While the insert search is open |
| Alt+Left / Alt+Right | Back / forward through jumps | Like a browser |
| Page Up / Page Down | Previous / next page | |
| Home | Zoom to fit page | Double middle click does the same |
| Ctrl+1 / 2 / 3 | Project, properties, messages panels | |
| Mouse wheel | Zoom at cursor | |
| Middle drag | Pan | |
| Right click | Context menu, first item repeats the last tool | |

Right drag should not pan by default: right click is the context menu in
EPLAN and in Windows generally, and a pan on right drag makes the menu fire
unreliably. If right-drag pan stays, it needs a movement threshold so a
right click still opens the menu.

## Journeys

### J1 New project

**Goal:** from the start screen to an empty schematic page with a title
block, ready to draw.

| Path | Actions | Typed |
| --- | --- | --- |
| NovoElectric target | 3 (Ctrl+N, type name, Enter) | name |
| EPLAN estimate | 10 to 15 (new project dialog, template, folder, project properties, new page dialog) | name, page data |

Defaults:

- Saved to `<Documents>\NovoElectric\<name>.novo` (per-user, no admin rights).
  `<Documents>` is the user's real Documents folder from the Windows
  known-folder lookup (`dirs::document_dir()`), never a hard-coded path,
  because company PCs often redirect Documents to OneDrive.
  A different folder is a field you can change, not a step you must take.
- Uses the last-used template, or the built-in IEC A3 template the first time.
- Creates page 1 (schematic, A3 landscape) and opens it, zoomed to fit.
- The start screen lists recent projects; one click or Enter opens the top
  one.

Acceptance:

- [ ] Ctrl+N, a name and Enter land on an open page 1 with a title block.
- [ ] No dialog asks for anything other than the name.
- [ ] Reopening the app offers the last project as the default action.

### J2 Place a device

**Goal:** put a symbol on the page as a tagged device.

| Path | Actions | Typed |
| --- | --- | --- |
| First placement | 3 (I, Enter, click) | about 3 (`con`) |
| Each further one of the same symbol | 1 (click) | 0 |
| EPLAN estimate | 8 to 12 (insert menu, symbol library tree, select, OK, place, properties dialog, OK) | tag |

Behaviour:

- I opens a search box at the cursor. It matches symbol names, synonyms
  (contactor, coil, K) and recently used symbols, recent first. Enter takes
  the top match. There is no separate insert centre to browse.
- Short, obvious mnemonics rank first: `no` normally open contact, `nc`
  normally closed, `coil`, `mcb`, `mpcb` motor protection breaker, `ol`
  overload, `m3` three phase motor, `pb` push button, `lamp`, `term`.
- The search box shows a row of nine favourites (the most used symbols,
  editable); 1 to 9 picks one, so common parts are I then one digit.
- The symbol follows the cursor with snap; R rotates and X mirrors before the
  click.
- Each click places a new device with the next free tag for the symbol's
  IEC 81346-2 letter code (`-K1`, `-K2`, ...), using the page's default
  function and location aspects. No properties dialog.
- If a pin lands on an existing wire, the wire is split and connected
  (auto-connect when points align).
- **Child symbols** (a contact of an existing contactor): typing the tag in
  the search (`K1 no`) places a normally open contact that belongs to `-K1`,
  with the next free contact numbers (13-14, then 23-24). Without a tag, the
  contact gets a new device and the checker flags it as having no coil.

Acceptance:

- [ ] Placing ten contactors in a row takes 3 actions plus 9 clicks.
- [ ] A favourite symbol is in hand after 2 actions (I, digit).
- [ ] Every placed device has a unique valid tag without user input.
- [ ] No dialog opens; the properties panel shows the new device.
- [ ] A contact placed with `K1 no` shows the coil's cross reference at once.
- [ ] One placement is one undo step.

### J3 Wire

**Goal:** connect two pins.

| Path | Actions | Typed |
| --- | --- | --- |
| One wire | 3 (W, click, click) | 0 |
| Each further wire | 2 (click, click) | 0 |
| T-junction onto an existing wire | same 2 clicks, end on the wire | 0 |
| EPLAN estimate | 2 to 4 per connection, plus a T-node symbol and direction choice per junction | 0 |

Behaviour:

- Click a pin, click another pin: an orthogonal route is drawn with at most
  two bends. Clicks on empty space add a corner.
- Ending on a wire creates the junction automatically; no junction symbol
  choice or direction dialog. The junction's direction follows from where
  the wires go.
- The wire inherits its potential and wire number from what it connects
  (WS-06 rules); conflicts show in the checker, not in a dialog.
- Hovering a pin highlights it before the click; a wire never ends on
  nothing by accident (a dangling end is flagged live).
- When a connection piece is wanted by hand, J picks it up and Tab cycles
  through all of them in one fixed order (four corners, four Ts, cross); R
  still rotates. No piece needs its own shortcut, so there is nothing to
  remember beyond J and Tab.
- A placed T-junction can be selected and re-oriented with Tab at any time;
  the wires follow.

Acceptance:

- [ ] Wiring a three-phase main circuit (contactor to overload, 3 wires)
      takes 1 + 6 actions.
- [ ] A T-junction never requires a choice from the user.
- [ ] Every corner and T orientation is reachable from J with Tab, on a
      new piece and on a selected one.
- [ ] Each wire is one undo step; Escape mid-wire removes only the
      unfinished wire.

### J4 Tag and renumber devices

**Goal:** give a device the tag the user wants, and keep tags tidy.

| Path | Actions | Typed |
| --- | --- | --- |
| Rename one device | 2 (F2, Enter) | the new tag |
| Renumber a page, selection or project | 3 (Ctrl+K, Enter on "Renumber devices", Enter to apply) | about 4 |
| Smart duplicate a motor circuit | 2 (select, Ctrl+D) or Ctrl+C and Ctrl+V on another page | 0 |
| EPLAN estimate | rename: properties dialog, 4 to 6 actions; renumber: offline numbering dialog with several settings pages | |

Behaviour:

- The project has one numbering scheme, chosen once (sequential per letter
  code like `-K1`, `-K2`, or page based like `-3K1` for page 3), and every
  new device follows it the moment it is placed.
- F2 edits the tag in place on the canvas. Typing only `-K5` keeps the page's
  function and location aspects. Tab moves between aspects when needed.
- Renaming a coil renames its contacts and every cross reference with it.
- A tag already in use is refused in the editor with the reason, not after
  committing.
- Renumber shows a preview of old and new tags in the palette, sorted by
  page and column, and applies as one undo step.
- Smart duplicate and paste give each copied device the next free number for
  its letter code and keep their internal relations (a copied coil and its
  contacts stay together).

Acceptance:

- [ ] Renaming `-K1` to `-K10` updates all its contacts and references.
- [ ] Renumbering a 200-device project is one undo step.
- [ ] Copying a motor circuit to a new page needs no manual renaming
      (WS-08 done criterion).

### J5 Terminals

**Goal:** put terminals in wires and get a terminal strip.

| Path | Actions | Typed |
| --- | --- | --- |
| A strip of N terminals across N wires | 2 (T, drag across the wires) | 0 |
| One terminal | 2 (T, click on the wire) | 0 |
| EPLAN estimate | per terminal: insert, place, properties dialog (strip and number), OK; or the terminal strip navigator | strip, numbers |

Behaviour:

- T then a drag across several parallel wires places one terminal per wire,
  in drag order, on the strip in use (`-X1`), numbered next free (`1`, `2`,
  ...).
- The strip in use is the last one used on the page, or a new `-X1`. Typing
  a strip tag right after T (`X2`) changes it.
- PE wires get a PE terminal automatically, N wires an N terminal.
- **When terminals connect is never a guess.** A terminal shows on the
  schematic whether it passes through (both sides connected) or is a
  single-sided end, and the checker flags a terminal whose sides are
  connected differently than its type allows.
- **Same name, same terminal.** A project setting decides what happens when
  the same terminal name (`-X2:1`) is placed in several spots. The default
  is "join them": the occurrences are one physical terminal, built from
  1-in/1-out terminals linked with a jumper or connector, and the strip
  editor shows them as one row with all their connections. The other option
  is "refuse duplicates", which flags the second occurrence.
- All terminals of one type on a strip share one style and one part, so 400
  terminals look and report the same; changing the type changes all of
  them.
- The terminal strip page and connection screen (WS-11) update live.
- Reordering, jumpers and spares are done in the strip editor table with
  keyboard editing, like a spreadsheet.

Acceptance:

- [ ] Five terminals on five motor wires take 2 actions.
- [ ] Terminal numbers are unique and continuous without user input.
- [ ] With "join terminals with the same name" on, placing `-X2:1` on two
      pages gives one terminal with both connections in the strip editor.
- [ ] A terminal's through or end state is visible on the schematic.
- [ ] The terminal screen shows internal and external targets with no extra
      step.

### J6 Cross-reference jump

**Goal:** go from a coil to its contacts and back.

| Path | Actions | Typed |
| --- | --- | --- |
| Coil to contact or back | 1 (G with the pointer or selection on it, or double-click the reference) | 0 |
| Back to where you were | 1 (Alt+Left) | 0 |
| All references of a device | 1 (Shift+G) | 0 |
| Link a cross reference by hand | 3 (right click, "Link cross reference", click the partner) | 0 |
| Find any device | 2 (Ctrl+K, Enter) | the tag |
| EPLAN estimate | 2 to 3 (right click, go to, pick in a list), and often no shortcut works | |

Behaviour:

- Coils show the contact image (pages and columns of each contact); contacts
  show where their coil is (`/3.4`). Both update live (WS-10).
- G on a coil with several contacts jumps to the first and shows a small
  list to cycle with G again; on a contact it jumps to the coil.
- Interruption points (off-page arrows) jump to their partner the same way.
- Every jump goes into a back/forward history.
- Shift+G always works on a selected device, coil or contact alike, and
  lists every occurrence with page and column; arrow keys and Enter jump.
- Selecting a coil highlights its contacts on the same page; contacts on
  other pages are listed in the status bar with their locations.
- Right click on a contact, coil or interruption point offers "Link cross
  reference"; the next click on the partner links them, and both
  references appear at once.
- Ctrl+K with a tag lists every place the device occurs.

Acceptance:

- [ ] G and Alt+Left round-trip between a coil and a contact on another
      page in 2 actions.
- [ ] Moving a contact updates the coil's contact image in the same frame.
- [ ] Selecting `-K1` highlights all its contacts on the page.
- [ ] Shift+G lists every occurrence of the selected device.

### J7 Cable

**Goal:** define a cable over wires and get its cable screen.

| Path | Actions | Typed |
| --- | --- | --- |
| Cable over N wires | 2 (C, drag across the wires) | 0 |
| Choose a cable type | +2 (Ctrl+2 or the panel field, Enter) | part search |
| EPLAN estimate | cable definition line, properties dialog, then core assignment per wire or via the cable editing dialog | |

Behaviour:

- C then a drag across wires creates a cable `-W1` (next free) with one core
  per crossed wire, in drag order.
- Cores take colours or numbers from the cable part when one is set, or a
  default numbering (1, 2, 3, PE for the protective conductor) when not.
- A shield and spare cores are set in the properties panel; the cable screen
  and overview (WS-12) update live.
- Dragging the same cable tool across wires on another page with the same
  cable selected adds cores to that cable instead of making a new one.

Acceptance:

- [ ] A four-core motor cable over L1, L2, L3 and PE takes 2 actions and
      gets PE on the protective conductor.
- [ ] The cable screen shows both ends and every core with no extra step.

### J8 Command palette and search

**Goal:** reach anything by name.

| Path | Actions | Typed |
| --- | --- | --- |
| Run any command | 2 (Ctrl+K, Enter) | 2 to 5 |
| Open any page | 2 | page number or name |
| EPLAN estimate | menu or ribbon hunt, 2 to 4 clicks if you know where | |

Behaviour:

- One search box for commands, pages (`3`, `Main circuit`), devices (`K1`)
  and symbols. Each row shows its kind and the shortcut if it has one, so the
  palette teaches the shortcuts.
- Recently used items rank first for an empty or short query.
- An exact tag (`-K1`, `K1`, `=A1+CAB1-K1`) puts that device first, with
  every location where it occurs (page and column) as rows to jump to.
- Menus and the palette agree: a command unavailable right now is greyed in
  both, with the reason on hover, rather than shown in one and missing from
  the other.

Acceptance:

- [ ] Every action in the menus is in the palette, and every palette command
      with a shortcut shows it.
- [ ] The last command used is the first row when the palette opens.
- [ ] Searching the exact name of any device returns its locations, every
      time.

### J9 Undo

**Goal:** take back a mistake without fear.

Acceptance:

- [ ] Ctrl+Z works in every view (schematic, strip editor, properties panel)
      and undoes the last user gesture as one step.
- [ ] Undo of a change on another page brings that page into view.
- [ ] Text fields keep their own Ctrl+Z while focused; the project undo
      takes over when focus leaves the field.
- [ ] Undo history survives an autosave and a crash recovery (WS-04).

### J10 Assign potentials

**Goal:** give wires their potential (L1, L2, L3, N, PE, PEN) and voltage.

| Path | Actions | Typed |
| --- | --- | --- |
| Three phase wires | 3 (select the three wires, right click, Enter on the suggestion) | 0 |
| A wire on a terminal named `X4:PE` | 0 (inferred) | 0 |
| EPLAN estimate | potential definition point or properties per wire, several dialogs | names |

Behaviour:

- Selecting three wires (a drag across them, or Shift-clicks) and right
  clicking puts "Three phase 400 V (L1, L2, L3)" first in the menu; the
  wires get L1, L2 and L3 in their order on the page. Other common systems
  (230 V L/N, 24 V DC +/-) follow.
- One wire suggests PE, N or PEN from what it touches: a terminal or pin
  named PE, N or PEN, or a wire already known to be one.
- A wire connected to a terminal named `X4:PE` is PE without asking; the
  same holds for N and PEN.
- Potentials flow through connected wires and through terminals; a
  conflict (PE meeting L1) is flagged live, not in a dialog.

Acceptance:

- [ ] Three phase assignment takes 3 actions.
- [ ] A wire to a terminal named `X4:PE` is PE with no user action.
- [ ] Changing the potential of one wire updates every wire on the same
      potential.

### J11 Edit text

**Goal:** move, rotate or change a single text without fighting the device
it belongs to.

| Path | Actions | Typed |
| --- | --- | --- |
| Move a text | 1 (drag the text) | 0 |
| Rotate a text | 2 (click the text, R) | 0 |
| Change a text | 2 (double-click, Enter) | the text |
| Restyle every text of one kind | 3 (right click, "Select same kind", change the style once) | 0 |
| EPLAN estimate | properties dialog, format tab, often several attempts | |

Behaviour:

- Clicking directly on a text selects that text alone; clicking the symbol
  selects the device. A dragged text stays attached to its device and moves
  with it afterwards.
- Double-click or F2 edits a free text in place; for a tag it edits the
  tag (J4).
- Text styles (font, size, angle, position relative to the symbol) come
  from the project style (principle 12). "Reset position" puts a moved text
  back where the style says.

Acceptance:

- [ ] A device tag can be moved, rotated and reset without opening a
      dialog.
- [ ] Changing the project's tag style changes every tag that has not been
      overridden.

### J12 Properties

**Goal:** find and change one property without a cryptic screen.

| Path | Actions | Typed |
| --- | --- | --- |
| Change one property | 3 (Ctrl+2, type part of its name, Enter, then the value) | a few letters and the value |
| Same change on many devices | same, with all of them selected | |
| EPLAN estimate | properties dialog, find the right tab among many, find the field | |

Behaviour:

- The properties panel is non-modal and edits the selection. The common
  fields for the selected kind come first (tag, description, part,
  potential); the rest are grouped and collapsed.
- A filter box at the top searches property names and values; typing
  "cross" finds every cross-reference setting.
- With several objects selected the panel shows shared values, and a change
  applies to all of them as one undo step.

Acceptance:

- [ ] Any property is reachable by typing part of its name.
- [ ] Editing 50 selected devices is one change and one undo step.

### J13 Cabinet layout (Phase 3)

**Goal:** get every device onto the mounting plate with warnings before
problems, not after.

Behaviour (WS-15):

- Devices with a part appear live in an "Unplaced" list; the list counts
  down as they are placed.
- "Place all" puts unplaced devices onto rails in schematic order with a
  preview the user accepts or adjusts; nothing moves without the preview.
- The layout warns ahead of time: a rail that will overflow, a device
  without dimensions, a duct over its fill limit.

Acceptance:

- [ ] Placing all devices of the Gate 1 project takes 3 actions (Ctrl+K,
      "Place all", accept).
- [ ] A rail that cannot hold its devices is flagged before placement.

## Scenario budget: Gate 1 motor starter

The Gate 1 circuit (direct-on-line starter, main circuit plus start/stop with
self-hold over 2 pages) is the end-to-end benchmark. WS-06's done criterion is
"drawn in under 5 minutes"; in actions the target is:

| Step | Actions |
| --- | --- |
| New project | 3 |
| Page 1: place -Q1, -K1, -F1, -M1 (I + search + click each) | 12 |
| Page 1: wire 3 phases through Q1, K1, F1 to M1 (W + 2 clicks per wire, 9 wires) | 19 |
| Page 1: PE to the motor | 2 |
| Page 1: terminals -X1 1 to 4 for the motor (T + drag) | 2 |
| Page 1: cable -W1 over the motor wires (C + drag) | 2 |
| New page 2 (Ctrl+K, "new page", Enter) | 3 |
| Page 2: place -S1, -S2, K1 coil, K1 hold contact, F1 contact | 15 |
| Page 2: wire the control circuit (about 7 wires) | 15 |
| **Total** | **about 73 actions, under 3 minutes for a practised user** |

A tester thread can check this by scripting the journey through the UI and
counting input events; any step that needs more than its budget is a
workflow bug.

## Review checklist for UI pull requests

- Does any new action open a modal dialog in the drawing loop?
- Is every new action in the palette, with a shortcut from the map above?
- Does Escape follow the ladder (step, tool, selection)?
- Is each gesture one undo step?
- Does a new object get a valid default (tag, number, position) without a
  question?
- Does the status bar say what the next click does?
- Do plain letter shortcuts act only while the canvas has focus, and never
  steal typing from a text field?
- Does a new panel or menu have a plain, distinct name (no "navigator")?
- Does the look of a new object come from a project style?
- Does selecting an object highlight what it is linked to?
- Does the right-click menu offer what fits the selection first?
- Does the journey stay within its budget?
