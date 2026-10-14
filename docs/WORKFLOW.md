# Workflow and interaction design

How NovoElectric works for the person using it: the interaction concept,
what goes where on screen, how modes and menus flow, and a budget of clicks
and keystrokes for every core task. The workflow thread owns this file and
leads the interaction concept. The layout thread leads the visual language,
and the main reviewer checks both. The three critique each other's
proposals before anything here is final.

NovoElectric is not EPLAN with fewer clicks. Tadjiro's EPLAN complaints
(listed at the end) are treated as symptoms that show where page-first CAD
breaks down, not as a feature list to copy.

## 1. How a designer thinks

The concept starts from the work, not from existing tools.

- **Designers think in functions, not sheets.** "Feed pump 3, 4 kW, direct
  on line, with a running signal to the PLC" is the unit of thought. Pages
  are how that function is printed.
- **They switch between four questions all day.** What connects to what
  (connectivity), where each thing sits (cabinet, field, desk), what each
  thing is (part) and how the drawing reads (layout). Page-first tools mix
  all four into one sheet and then need a navigator for each.
- **Most work is repetition with variation.** Twenty motor feeders that
  differ in power and tag. Copying frozen macros and renaming them by hand
  is where hours go.
- **Drawing is thinking.** Placing and connecting must stay direct and
  immediate. A form is not a substitute for a pen.
- **Big edits are data edits.** Renaming 400 terminals or setting 60 cable
  types is spreadsheet work, and should be done in a table, not one dialog
  at a time.

Page-first CAD treats the sheet as the truth and derives intelligence from
symbols on sheets. That one choice causes most of the list at the end: a
navigator per kind of object, macros that go stale after placement, manual
cross-reference linking, wires that must be told their potential, terminals
placed by hand, and a build step to bring reports up to date.

## 2. The concept in five ideas

### 2.1 One model, many lenses

The project is one electrical model (WS-02). What the user sees are
**lenses** onto it:

| Lens | Shows | Its outline (left) |
| --- | --- | --- |
| Schematic | Pages of circuits | Functions (`=`) and pages |
| Connections | Terminal strips and cables | Strips, cables, plugs |
| Devices | Every device and its part | Locations (`+`) and device kinds |
| Cabinet | Mounting plates and rails | Enclosures and plates |
| Reports | Generated lists and pages | Report types |

- **The selection travels with you.** Select `-K1` on a schematic, switch to
  Cabinet, and `-K1` is selected on the plate. Switch back and you are where
  you were.
- **Every lens has a drawing face and a table face.** Ctrl+T flips between
  them on the same objects with the same selection. Bulk work happens in the
  table: sorting, filtering, fill-down, multi-cell paste and find-and-replace
  with patterns (`X2:*` to `X3:*`).
- There is one outline panel whose contents follow the lens. It replaces
  every "navigator".

### 2.2 Say what, not how

The user states intent and the program does the bookkeeping.

- **Connections, not line pieces.** The user connects pin to pin. Routing,
  corners, T-junctions and junction dots are how a connection is drawn,
  not objects to place and rotate. A junction dot can be dragged to tidy
  the drawing; it never needs choosing or orienting.
- **Potentials come from sources.** A supply (an incoming terminal, a
  transformer, a power supply, a supply block) declares "3~ 400 V + N + PE"
  or "24 V DC". The potential flows through every conductor, contact and
  terminal it reaches. Wires are never assigned one by one. PE, N and PEN
  are recognised from pin and terminal names.
- **Terminals and cables are proposed where connections cross a location
  boundary.** If `-K1` is in `+CAB1` and `-M1` is in `+FIELD`, the
  connections between them have to leave the cabinet. The program proposes
  the terminals (`-X1:1` to `-X1:4`, PE included) and a cable (`-W1`, 4
  cores) and shows the proposal on the drawing. The user accepts it with
  one key, or edits it first.
- **References follow ownership.** A contact belongs to a device. Give it
  the device (type `K1` in its tag, or drop it on the coil) and every cross
  reference appears. Nothing is linked by hand.

### 2.3 Rules, not chores

Every repeated decision is made once, as a project rule, on one Rules page
written in plain sentences:

- "Devices are numbered **per letter code** (`-K1`, `-K2`)" or "**per
  page** (`-3K1`)".
- "Terminals with the same name are **one terminal**, joined with
  **jumpers**".
- "Device tags are **2.5 mm**, **above right** of the symbol".
- "Wires are numbered **by potential**" or "**by page and column**".
- "PE is drawn **green-yellow**, dashed".

Changing a rule updates the whole project immediately. An instance that
differs from a rule is an **override**, marked in the inspector, counted in
the status bar ("3 overrides") and resettable with one action. This is why
400 terminals look identical: they follow one rule unless someone
deliberately overrides one.

### 2.4 Circuits are live blocks

The common unit of work is a circuit, so circuits are first-class:

- A **block** is a parametric circuit: "Motor feeder, DOL" with power,
  voltage, control voltage and PLC signals as parameters. Placing it creates
  the devices, connections and parts (sized from the power), and the
  terminals and cables follow from 2.2.
- Blocks **stay linked**. Changing a parameter on one instance (4 kW to
  7.5 kW) re-sizes its parts. Changing the block offers the update to every
  instance, with a preview. An instance can be detached to edit freely.
- Any selection can be saved as a block; tags are renumbered on every
  placement.

### 2.5 One command bar

A single command bar sits at the bottom of the window, always visible.
Space (with the drawing focused) or Ctrl+K (anywhere) focuses it.

- **A noun selects or jumps:** `K1` selects `-K1` and lists everywhere it
  occurs; `12` opens page 12; `X2:1` finds that terminal.
- **A verb with a noun acts:** `no K1` places a normally open contact of
  `-K1`; `dol 4kW` places a motor feeder block; `connect K1:14 S2:3`;
  `renumber page`.
- **A bare verb** runs a command: `rules`, `export pdf`.
- Results preview live on the drawing before Enter. Each row shows its
  shortcut, so the bar teaches the keys.

Menus become a browsable map of the same commands: **Project, Edit, View,
Help**. There is no ribbon and no insert centre.

### And: problems show where they are

There is no check run. A problem (an unconnected pin, a duplicate tag, a
coil with too many contacts, a device that will not fit its rail) appears as
a small mark at its place on the drawing and as a count in the status bar.
Proposals (terminals to accept, a part to choose) use the same channel. The
program warns before a problem exists ("this rail will overflow") rather
than after.

## 3. Where things live

```
+------------------------------------------------------------------+
| Project name  ·  Schematic  ›  =PUMPS  ›  page 12        (thin)  |
+------+-----------------------------------------+-----------------+
| Lens | Outline     |                           | Inspector       |
| rail | (follows    |   Drawing or table        |  -K1 Contactor  |
|      |  the lens)  |                           |  Relations      |
|  Sch |             |                           |  Fields         |
|  Con |             |                           |  [search props] |
|  Dev |             |                           |                 |
|  Cab |             |                           |                 |
|  Rep |             |                           |                 |
+------+-------------+---------------------------+-----------------+
| > command bar                   hint · 2 problems · 3 overrides  |
+------------------------------------------------------------------+
```

- **Lens rail (far left):** the five lenses as named icons. Alt+1 to Alt+5.
- **Outline:** the tree for the current lens. It filters as you type.
- **Centre:** the drawing face or the table face of the lens.
- **Inspector (right):** whatever is selected. **Relations** come first:
  what it connects to, where its contacts are, which strip, cable, plate
  and block it belongs to, each one clickable. Then the few fields that
  matter for that kind, then a search box over every other property. With
  several objects selected it edits them all as one undo step.
- **Bottom:** the command bar, the hint for the current tool ("Click the
  second pin, Esc to stop"), and counters for problems and overrides.
- **Context menu:** built from the selection, best guess first, and its
  first entry repeats the last tool.

The layout thread decides how this looks (dark theme, density, iconography).
This section only fixes what is where and why.

## 4. Modes

Select is the default and is never more than one Escape away.

- **Escape ladder:** cancel the step in progress, then leave the tool, then
  clear the selection. It works whenever the drawing has focus.
- **Tools are short-lived and sticky.** Place and Connect stay active for
  the next click until Escape.
- **Direct manipulation first:** dragging from a pin draws a connection
  without picking a tool, and dragging a text moves only that text.
- **Single-letter keys act only while the drawing has focus** and never take
  typing from a text field.
- **One gesture, one undo step**, including a renumber of 400 tags or
  accepting a terminal proposal. Undo never asks a question.
- **No save prompts.** Autosave (WS-04) makes Ctrl+S a reassurance.

## 5. Shortcut map

| Key | Action | Scope |
| --- | --- | --- |
| Space | Focus the command bar | Drawing |
| Ctrl+K | Focus the command bar | Anywhere |
| Alt+1 to Alt+5 | Schematic, Connections, Devices, Cabinet, Reports | Anywhere |
| Ctrl+T | Drawing face / table face | Anywhere |
| Ctrl+N / Ctrl+O / Ctrl+S | New, open, save | Anywhere |
| Ctrl+Shift+S / Ctrl+W / Ctrl+Q | Save as, close project, quit | Anywhere |
| Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) | Undo / redo | Anywhere |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy, cut, smart paste (next free tags) | Anywhere |
| Ctrl+Shift+V | Paste with the original tags | Anywhere |
| Ctrl+D | Smart duplicate | Drawing |
| Ctrl+A | Select all in view | Anywhere |
| Ctrl+H | Find and replace (table face) | Table |
| Ctrl+1 / 2 / 3 | Outline, inspector, problems panels | Anywhere |
| Delete | Delete selection and its dangling connections | Drawing |
| Enter | Accept the highlighted proposal, or repeat the last tool | Drawing |
| F2 | Edit the selected tag or text in place | Drawing |
| I | Place: opens the command bar in place mode at the cursor | Drawing |
| W | Connect (also: drag from a pin) | Drawing |
| M | Move (also: drag) | Drawing |
| R / X | Rotate 90° / mirror, while placing or on the selection | Drawing |
| G | Go to partner (coil, contact, off-page arrow) | Drawing |
| Shift+G | List every occurrence of the selected device | Drawing |
| Alt+Left / Alt+Right | Back / forward through jumps | Anywhere |
| Page Up / Page Down | Previous / next page | Drawing |
| Home | Zoom to fit (also: double middle click) | Drawing |
| Wheel / middle drag | Zoom at cursor / pan | Drawing |
| Right click | Context menu (right drag pans after 4 px) | Drawing |

There are no keys for corners, T-pieces, terminals, cables or potentials,
because those are not things the user places.

## 6. Journeys and budgets

**Counting:** an action is one click, one key press, one key chord or one
drag. Typed content (a tag, a search word) is counted separately. A budget
is a ceiling for the default path; anything slower fails review.

### J1 New project

Budget: 3 actions (Ctrl+N, a name, Enter), and the name typed.

- Saved to `<Documents>\NovoElectric\<name>.novo`. `<Documents>` comes from
  the Windows known-folder lookup (`dirs::document_dir()`), so OneDrive
  redirection works and no admin rights are needed.
- The project starts with the last-used template's rules, or the IEC
  defaults, and opens page 1 of the Schematic lens.
- The start screen lists recent projects; Enter opens the top one.

### J2 Place a device

| Path | Actions | Typed |
| --- | --- | --- |
| First device | 3 (I, Enter, click) | about 3 (`coil`) |
| Each further one | 1 (click) | 0 |
| A contact of an existing device | 3 (I, Enter, click) | `no K1` |

- The command bar's place mode ranks short names first (`no`, `nc`, `coil`,
  `mcb`, `mpcb`, `ol`, `m3`, `pb`, `lamp`), then blocks, then recent
  symbols. Nine favourites are on 1 to 9.
- Each click places a device with the next free tag by the numbering rule.
  There is no dialog.
- A pin that lands on a connection splits it and connects.

### J3 Connect

| Path | Actions |
| --- | --- |
| One connection | 1 (drag from pin to pin) |
| A chain | W, then 1 click per pin |
| Onto an existing connection (T-junction) | 1 (drag from the pin onto the line) |

- The route is orthogonal with at most two bends and avoids symbols.
  Clicks on empty space while connecting add a fixed corner.
- Junctions and corners are created and drawn by the program. Dragging a
  junction dot moves it. There is nothing to choose or rotate.
- Escape mid-connection drops only the unfinished connection.

### J4 Tags and numbering

| Path | Actions | Typed |
| --- | --- | --- |
| Rename one device | 2 (F2, Enter) | the tag |
| Renumber a page, selection or project | 3 (Space, `renumber page`, Enter) | the command |
| Change the numbering rule | 3 (Space, `rules`, pick) | |

- Typing only `-K5` keeps the function and location of the page.
- Renaming a coil renames its contacts. A duplicate tag is refused while
  typing, with the reason.
- Renumber previews old and new tags and applies as one undo step.

### J5 Terminals and cables

| Path | Actions |
| --- | --- |
| Accept the proposed terminals and cable for a motor | 1 (Enter on the proposal) |
| Accept every open proposal on the page | 2 (Space, `accept all`) |
| Edit a proposal first (other strip, cable type) | in the inspector, then Enter |

- Proposals appear as soon as a connection crosses a location boundary;
  each is drawn ghosted on the schematic and listed in the problems panel.
- Terminals follow the strip rules: same-name terminals (`-X2:1` placed
  twice) are one terminal joined by a jumper, and PE and N get PE and N
  terminals.
- Each terminal shows whether it passes through or is single-sided.
- Strip order, levels, jumpers and spares are edited in the Connections
  lens, in its table face, by keyboard.
- Cables take core colours or numbers from the cable part; PE always goes
  on the protective core.
- Terminals can still be placed by hand (`term` in the command bar) where no
  boundary exists.

### J6 Cross references

| Path | Actions |
| --- | --- |
| Coil to contact or back | 1 (G) |
| Back to where you were | 1 (Alt+Left) |
| Every occurrence of a device | 1 (Shift+G) |
| Give a stray contact its coil | 1 (drop it on the coil) or F2 and the tag |

- Coils show their contact image and contacts show their coil location,
  live (WS-10).
- Selecting a device highlights everything it switches on the page and
  lists the rest under Relations in the inspector.

### J7 Potentials

| Path | Actions |
| --- | --- |
| Declare a supply | 3 (select the incoming terminals or supply, choose "3~ 400 V + N + PE" in the inspector) |
| Wire on terminal `X4:PE` | 0 (recognised) |
| Override one conductor | 2 (select it, change it in the inspector) |

- Potentials flow from the source through conductors, contacts and
  terminals, and colour the connections by rule.
- A conflict (L1 meeting PE) is marked where it happens.

### J8 Command bar and search

| Path | Actions | Typed |
| --- | --- | --- |
| Any command | 2 (Space, Enter) | 2 to 5 |
| Any device, with every place it occurs | 2 | the tag |
| Any page | 2 | number or title |

- An exact tag (`K1`, `-K1`, `=A1+CAB1-K1`) always returns that device
  first with every location (page and column).
- Recent items rank first. Menus and the command bar always list the same
  commands, with the same enabled state and reason.

### J9 Undo

- Ctrl+Z undoes the last gesture as one step in every lens and face, and
  brings the affected page into view.
- Text fields keep their own Ctrl+Z while focused.
- Undo history survives autosave and crash recovery (WS-04).

### J10 Text

| Path | Actions |
| --- | --- |
| Move one text | 1 (drag it) |
| Rotate it | 2 (click, R) |
| Edit it | 2 (double-click or F2, Enter) |
| Reset it to the rule | 2 (click, the reset mark in the inspector) |

- Clicking a text selects the text alone; clicking the symbol selects the
  device. The text stays attached to its device.

### J11 Inspector and tables

| Path | Actions | Typed |
| --- | --- | --- |
| Change one property | 3 (Ctrl+2, type its name, Enter) | a few letters and the value |
| Same change on 50 devices | the same, with them selected | |
| Rename 400 terminals by pattern | 3 (Ctrl+T, Ctrl+H, Enter) | the pattern |

- Relations first, then the few fields that matter, then a search box over
  the rest. Multi-selection edits are one undo step.

### J12 Circuit blocks

| Path | Actions | Typed |
| --- | --- | --- |
| Place a 4 kW DOL motor feeder | 4 (Space, Enter, Tab to power, click) | `dol 4kW` |
| Change it to 7.5 kW | 2 (select, edit in the inspector) | the value |
| Save a selection as a block | 3 (select, Space, `save block`) | a name |

- Placing a block gives devices, connections and sized parts, and
  terminal and cable proposals where it crosses locations.
- Editing a block offers the update to every instance, with a preview.

### J13 Cabinet

- Devices with a part appear live under "Unplaced" in the Cabinet outline.
- "Place all" previews the placement on the rails before anything moves.
- Rails that will overflow, missing dimensions and duct fill are warned
  about before placement, in the problems channel.
- Budget: 3 actions to place every device of the Gate 1 project (Space,
  `place all`, Enter).

## 7. Gate 1 benchmark

Gate 1 is a direct-on-line motor starter (main circuit and start/stop with
self-hold) over two pages.

| Step | By hand | With a block |
| --- | --- | --- |
| New project | 3 | 3 |
| Supply: declare 3~ 400 V + N + PE on the incoming terminals | 3 | 3 |
| Place -Q1, -K1, -F1, -M1 | 12 | 4 (`dol 4kW`, J12) |
| Connect the three phases and PE (10 drags) | 10 | (in block) |
| Accept terminals -X1 and cable -W1 | 1 | 1 |
| New page | 3 | (in block) |
| Place -S1, -S2, K1 coil, K1 hold contact, F1 contact | 15 | (in block) |
| Connect the control circuit (about 7 drags) | 7 | (in block) |
| **Total** | **54 actions** | **11 actions** |

The earlier draft of this spec, which mirrored EPLAN's tools, budgeted 73.
A tester thread can script these journeys and count input events; any step
over budget is a workflow bug.

## 8. What changed from the earlier draft, and why

The first drafts answered EPLAN complaints one by one, and several answers
kept EPLAN's model with a shortcut bolted on. These were rejected:

| Earlier draft | Now | Why |
| --- | --- | --- |
| J key and Tab to cycle corner and T pieces | No connection pieces; junctions are drawn by the program | The pain is having to manage T pieces at all, not the key to rotate them |
| Select L1, L2, L3, right click, "3~ 400 V" | Potentials declared at the source and flowing | Assigning wires is a chore that the source already answers |
| T tool to place terminals across wires | Terminals and cables proposed at location boundaries | Terminals are a consequence of where devices are |
| C tool to drag a cable across wires | Cable proposed with the terminals | Same reason |
| Right click, "Link cross reference" | Contacts belong to devices; references follow | Linking by hand only exists because the model was page-first |
| Command palette as a pop-up | Always-visible command bar with verb and noun | Search, insert and commands are one habit, not three |
| Frozen snippets with smart paste | Live parametric blocks | Repetition with variation is the core job |
| Properties panel with a filter | Inspector with Relations first, plus a table face | Most questions are "what is this connected to", and bulk work belongs in a table |

## 9. Principles for every UI pull request

1. No modal dialog in the drawing loop.
2. Defaults first: every new object gets a valid tag, number and position
   without a question.
3. State intent, let the program do the bookkeeping (routing, junctions,
   potentials, terminals, references).
4. Rules over instances: looks and numbering come from project rules;
   overrides are visible and resettable.
5. One model, many lenses; the selection travels between them.
6. Everything is in the command bar; menus are a map of it.
7. Problems and proposals show where they are, before they become problems.
8. One gesture, one undo step; no save prompts.
9. Plain names for every panel; no family of navigators.
10. Live everywhere: no build or update step. A 50-page project opens in
    under 2 seconds, and generated pages update within a second.

## 10. Review checklist

- Does the change add a dialog, a mode or a key where intent and a rule
  would do?
- Does it add an object the user must place or rotate that the program
  could derive?
- Is the new command in the command bar and the menus, with its shortcut?
- Does Escape follow the ladder, and is each gesture one undo step?
- Do letter keys act only while the drawing has focus?
- Does the selection survive a lens switch?
- Does a new look come from a rule, with overrides visible?
- Does the journey stay within its budget?

## 11. Symptoms from EPLAN this answers

Tadjiro's list (2026-10-14), each traced to the idea above that removes the
cause.

| Symptom in EPLAN | Answered by |
| --- | --- |
| Hundreds of panels all called "navigator" | 2.1 one outline that follows the lens |
| Single texts hard to move, rotate, change | J10 |
| Texts and 400 identical terminals look different | 2.3 rules and overrides |
| Unclear when terminals connect | 2.2 boundary proposals, J5 through/end state |
| Same-named clamps should join through 1-in/1-out terminals | 2.3 strip rule |
| Layout generator manual, no heads-up | J13, problems channel |
| Potential assignment sluggish | 2.2 potentials from sources, J7 |
| Numbering painful | 2.3 numbering rule, J4 |
| Long build times | Principle 10 |
| Cryptic properties screen | J11 inspector, table face |
| T-pieces cannot be rotated; scattered corner shortcuts | 2.2 no connection pieces |
| Cross references hard to link | 2.2 ownership, J6 |
| No reliable shortcut to all references | J6 Shift+G |
| Search cannot find a device by exact name | J8 |
| Selecting a relay does not highlight what it switches | J6, inspector Relations |
| Insert centre hunt for common parts | 2.5 command bar, J2, J12 blocks |

## 12. Open questions

These are being decided in the critique loop and do not block work:

- Should pages flow automatically (a full page continues on the next, with
  off-page arrows created for you), or stay hand-made?
- How much routing should be automatic before it fights the designer's own
  layout of a circuit?
- Should blocks be stored in the project, in a shared company library, or
  both?
