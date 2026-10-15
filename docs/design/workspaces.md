# Workspaces, menus and navigation

How the window is organised now that KoperCAD has more than a schematic
editor: a Cabinet with a table and a 3D view, reports, commissioning, a
parts browser, macros, spreadsheets, revisions and checks. This extends
`docs/DESIGN.md` §3 (the Workbench) and §4 (Commands). **It reverses three
decisions in DESIGN.md §8 and §9** (no menu bar, no status bar, reports are
only commands), for the reason given in §1. Everywhere else DESIGN.md wins.

Owner of this page: the Workspaces thread. Visual language is
`docs/design/ui-guidelines.md` (Layout and branding thread); keys belong to
the Workflow thread; the shell code belongs to WS-01.

Status: design. Nothing here is built yet; §10 is the order it lands in.

## 1. Why

Tadjiro, 2026-10-15: "Lots of features are being built, but are these
integrated into the program through workflow and UI?"

An audit of `main` (074138a) says the answer is: partly. The window has a
mark menu with ten entries, a Schematic | Cabinet switch, a drawer with three
tabs and a command field. Everything else is reachable only by knowing its
name:

- The 3D view, Reports, Commissioning, the Parts window, Macros and Sheets
  (Excel round trip) have no place in the window. Their designs each ask the
  question "where does this live?" and leave it open (`3d-view.md` §12,
  `commissioning.md` "Commissioning mode in the app").
- There are two command lists that do not know each other: `Action` in
  `novo-app` (17 entries, the mark menu) and `COMMANDS` in `novo-workflow`
  (about 75 entries, the command field). A feature that is added to one is
  not in the other.
- Ctrl+3, Ctrl+4 and Ctrl+5 are spoken for by the drawer tabs, so the next
  three workspaces have no key.
- No panel remembers whether it was open. Opening the Cabinet and coming back
  leaves the drawer wherever the last screen put it.

The reason DESIGN.md gave for having no menu bar was that it is "a second,
worse index of commands". That is true of a hand-written menu. A menu that is
generated from the command registry is the same index drawn in a place a new
person looks first, and it is how someone who has never seen the program finds
out that Reports exist. The command surface stays the primary way in
(Ctrl+K); the menu bar is how you learn what you can ask it. The status bar
returns for the same reason: the HUD, the saved state, the active modes and
the last message are four small things in four places today.

### Principles

1. **One registry.** Every command is a row in the command registry. The
   menu bar, the command surface, the keymap, the shortcut overlay and the
   context menus are views of it. A command that is not in the registry does
   not exist; a menu entry that is not a command does not exist.
2. **A feature has a home.** Every feature lives in exactly one workspace or
   one window, and has a command that opens it. A feature design that cannot
   name its home is not ready for review (§9, the checklist).
3. **The drawing does not move.** Switching workspace never moves the page
   under the pointer, and returning restores the panels as they were left.
4. **Selection travels.** A device selected in one workspace is selected in the
   next, and shown there (§6).
5. **Quiet at rest.** Panels you did not ask for are closed; the menu bar
   collapses on narrow windows; Focus mode hides all of it.

## 2. Vocabulary

| Word | Meaning | Examples |
| --- | --- | --- |
| **Workspace** | What the canvas area shows, which panels are its home, and whether the drawing can be edited. One at a time, chosen in the switcher. | Schematic, Cabinet, 3D, Reports, Commissioning |
| **Face** | A second way to look at the same workspace data, chosen in a small segment inside the workspace. | Cabinet: Plate or Table |
| **Mode** | An overlay on a workspace that adds marks and changes no panel. A chip in the status bar shows it. | Live potentials (Shift+L), Changes, Marks (Shift+P) |
| **Panel** | A pane in the window frame with a home zone. | Outline, Inspector, Drawer |
| **Window** | A floating window for work outside the drawing loop. At most one is open at a time. | Parts, Preferences, Project settings, Rules, Issue revision, Macros |

Rule of thumb for new features: if it needs its **own canvas or table in the
middle** or it **must not be edited by accident**, it is a workspace. If it
**paints on the drawing you already have**, it is a mode. If it is **a form
you fill in and close**, it is a window. If it is **a table beside the
drawing**, it is a drawer tab.

Commissioning is a workspace although its design calls it a mode: it needs its
own panel, it recolours the whole drawing and it switches editing off, and
someone standing at the machine must not be one stray click from moving a wire.
Changes and Live are modes: they paint on the schematic and leave editing on.

## 3. The workspaces

| # | Workspace | Canvas | Edits the model | Key |
| --- | --- | --- | --- | --- |
| 1 | **Schematic** | The current page (`novo-canvas`) | Yes | Ctrl+1 |
| 2 | **Cabinet** | The mounting plate of the current panel; faces **Plate** and **Table** | Yes (placements) | Ctrl+2, Ctrl+T switches face |
| 3 | **3D** | The same panel in 3D (`novo-view3d`) | No (select, orbit, open door) | Ctrl+3 |
| 4 | **Reports** | The selected report as a live table or sheet | No (export, and add as generated pages) | Ctrl+4 |
| 5 | **Commissioning** | The current schematic page, read-only, coloured by check state | Only check results and the punch list | Ctrl+5 |

Ctrl+Tab and Ctrl+Shift+Tab cycle the workspaces in this order. The switcher
is a segmented control in the top bar (`widgets::segment`, icons from
`novo_theme::icons::canvas`); below 1100 px it shows icons only, with the name
in the tooltip.

**3D is a workspace, not a face of the Cabinet.** `3d-view.md` §12 proposed a
third face next to Plate and Table and left the answer to the owner. The
differences decide it: 3D has no table, no drawer and no placement editing, it
wants the whole window, and it has its own camera and view cube. It still
reads the same panel and selection as the Cabinet and the two stay in step
(§6). `3d-view.md` is updated to say so in the PR that builds the 3D
workspace.

A workspace that is not usable yet is **shown, not hidden**: its switcher chip
is disabled and the tooltip says why ("No cabinet yet. Cabinet › New cabinet").
A switcher whose entries come and go teaches nothing.

### 3.1 Schematic

What exists today, unchanged: Outline (Pages, Issues, Changes), canvas, tool
dock, HUD (moving to the status bar, §7), Inspector overlay, drawer with
Terminals, Cables, Devices. New: the drawer gains **I/O** (PLC cards and
channels, from `novo-plc`) and the Parts window opens from Tools.

### 3.2 Cabinet

The panel is chosen in the Outline, whose mode becomes **Panels** here (the
locations of the project, each with its device count and its problem count).
The header strip's panel picker goes away; the Outline is the one place that
lists structure.

| Zone | Content |
| --- | --- |
| Left | Outline in Panels mode |
| Centre | Plate, or Table face (Ctrl+T) |
| Right | Inspector: placement, footprint, clearances, power loss, mounting |
| Drawer | **Unplaced** (opens here by default, as today), **Problems** (layout checks: collisions, clearance, door swing, depth), **Thermal** |

`Place all` and its preview stay on the canvas as today.

### 3.3 3D

| Zone | Content |
| --- | --- |
| Left | Outline in Panels mode (same as Cabinet) |
| Centre | The 3D view with the view cube, F to frame, Home to fit |
| Right | Inspector, read-only |
| Drawer | closed; **Unplaced** and **Problems** are available |

Clicking a device selects it, exactly as in the Cabinet. The command
**Show in 2D** (`cabinet.show_2d`) returns to the Cabinet with the selection
centred. Esc never leaves the workspace (`3d-view.md` §6), and double-click
still fits the item.

### 3.4 Reports

Today reports are generated pages inside the schematic set. They stay that
way (DESIGN.md §8, pinned flow), and this workspace is how you **look at them
and build them without hunting through the page list**.

| Zone | Content |
| --- | --- |
| Left | The report list, grouped: **Sheets** (Contents, Terminal diagram, Cable diagram, Plate drawing), **Lists** (BOM, Device list, Wire list, Cut list, Cable schedule, Plug connection list, Drill table), **Calculations** (Thermal, Voltage drop), **Commissioning report**. Each row shows its state: on in the project, off, or stale. |
| Centre | The selected report, live. Lists are sortable tables; sheets are the generated page. |
| Right | Scope and columns for the selected report: whole project, one panel, one strip, one revision; the columns shown. Remembered per report in the project. |
| Header | **Export** (PDF, Excel, CSV), **Add to the drawing set** (turns the kind on, DESIGN.md §8), **Sheets…** (the Excel round trip, `spreadsheets.md`) |

Selecting a row in a table selects the device or wire in the model, so
switching to Schematic jumps there. The reports are produced by `novo-reports`;
this workspace only hosts them and has no report logic of its own.

### 3.5 Commissioning

Follows `commissioning.md`; this page only fixes where things sit.

| Zone | Content |
| --- | --- |
| Left | Outline in Pages mode, with a progress ring per page |
| Centre | The page, read-only, items coloured by state (open, passed, failed) |
| Right | **Commissioning panel**, pinned: the selection's checks, progress, who and when |
| Drawer | **Punch list** |

Editing the drawing is off. A banner in the status bar reads "Read-only
while commissioning". The report is made from Reports › Commissioning report.

## 4. The window frame

```
+--------------------------------------------------------------------------------+
| mark  File Edit Draw View Check Reports Tools Help | Schematic Cabinet 3D ... |  42 px
|       breadcrumb                           [Search or run  Ctrl+K]  issues  (!) |
+---------------+----------------------------------------------------+-----------+
| Outline       |                                                    | Inspector |
| (left zone)   |            canvas / workspace content              | (right)   |
+---------------+----------------------------------------------------+-----------+
| Drawer tabs: Terminals | Cables | Devices | I/O      (bottom zone)             |
+--------------------------------------------------------------------------------+
| status message | selection summary            modes   grid snap zoom   Saved    |  22 px
+--------------------------------------------------------------------------------+
```

Zones, one panel kind per zone, so a panel's place is never a surprise:

| Zone | Panels | Behaviour |
| --- | --- | --- |
| Top bar | Mark, menus, workspace switcher, breadcrumb, command surface, issue pill | 42 px, always shown except in Focus mode |
| Left | **Outline** only. Its modes differ per workspace (Pages, Issues, Changes, Panels, Reports) | 248 px; pinned above 1700 px, a flyout below, as today |
| Right | **Inspector** | 276 px; overlay by default, pin makes it a docked panel. The Commissioning panel uses this zone, pinned |
| Bottom | **Drawer** tabs | At most 35% of the height, docks right above 1800 px wide, as today |
| Canvas floating | Tool dock, Live card, Changes card, toasts | Float over the canvas and never take space |
| Status bar | Message, selection, modes, HUD values, saved state | 22 px; hidden in Focus mode and by a setting |
| Windows | Parts, Preferences, Project settings, Rules, Issue revision, Macros, Walk-through | Centred, one at a time, opened from a command and closed with Esc |

What this adds to DESIGN.md §3 is the **right zone as a rule** (only the
Inspector and the Commissioning panel live there), the **status bar**, and
the **menus**. The rest is unchanged, including the screen-size table; the
status bar takes 22 px, so the canvas share at 1366x768 with the drawer
closed is 704 of 768 px (92%) where DESIGN.md said 95%. The 95% target is
retired for 92%, and Focus mode still gives 100%.

### 4.1 Per-workspace layout, remembered

Each workspace remembers its own layout in the user's settings
(`Settings.workspaces`, per user, not in the project file, because it
describes the person's screen):

```
WorkspaceLayout {
    outline: Pinned | Flyout | Hidden,   // None = the width rule
    outline_mode: OutlineMode,
    drawer: Option<DrawerTab>,           // None = closed
    drawer_dock: Bottom | Right,         // None = the width rule
    inspector_pinned: bool,
}
Settings { last_workspace: Workspace, workspaces: map Workspace -> WorkspaceLayout }
```

- Switching restores the target's layout; leaving saves the current one.
- The first time a workspace opens it gets its default from §3.
- A new project opens in the workspace it was left in, with the same page and
  zoom (DESIGN.md §3, Start-up).
- **Reset layout** (command `view.reset_layout`, View menu) returns the
  current workspace to its default, and **Reset all layouts** returns all.
- Unknown fields and workspaces in an old settings file are ignored; a missing
  entry is the default. Nothing in a layout can make the window unusable, so a
  layout that would hide every way back (no top bar, no status bar, no
  Ctrl+K) cannot be stored.

## 5. Menus

### 5.1 The registry

`CommandInfo` (in `novo-workflow`, `commands.rs`) gains one field,
`menu: Menu`, which is a menu name plus an optional submenu. The groups
(Project, Draw, Edit, Navigate, View, Check, Export) stay as the command
surface's headings. Unit tests hold the invariants:

1. Every `Action` in `novo-app` is a `CommandInfo` with the same id, and the
   mark menu is gone as separate code (the two lists become one).
2. Every command has a menu, except those listed in `PALETTE_ONLY` with a
   reason (for example `tool.repeat`, which is a key and a context-menu row).
3. Every command that opens a workspace, panel or window is in the View or
   Tools menu with its key.
4. Every key in the keymap and every shortcut in the shortcut overlay is a
   command in the registry.
5. Command ids are unique and every label is unique within its menu.

A feature PR that adds a command and no menu row fails (2); that is the
enforcement of "a feature has a home".

### 5.2 The menus

Entries with a key show it right-aligned. Disabled entries stay visible and
say why in the tooltip (`availability`, as in the command surface). A trailing
`…` means the entry opens a window.

| Menu | Entries |
| --- | --- |
| **File** | New project · Open… · Open example · Recent ▸ · Save · Save as… · Close project · **Import ▸** (DXF as background, EPLAN parts, Sheets…) · **Export ▸** (PDF, DXF, Excel, CSV) · Project settings… · Rules… · Standard ▸ · Preferences… · Quit |
| **Edit** | Undo · Redo · Cut · Copy · Paste · Paste keeping tags · Duplicate · Select all · Delete · Rename · Renumber ▸ (page, interruption points, I/O) |
| **Draw** | Place… · Connect · Cable · Text · End on part · Next pin · Wiring mode ▸ (Corners, Lines) · Tidy · Accept proposal · Accept all · Link contact · **Terminals ▸** (connect, disconnect, toggle PE, strip order) · **PLC ▸** (add card, to PLC) · **Macros ▸** (insert, save selection as macro, library…) |
| **View** | **Workspace ▸** (Schematic, Cabinet, 3D, Reports, Commissioning) · **Panels ▸** (Outline, Inspector, each drawer tab) · Zoom to fit · Marks · Override marks · Live potentials · Changes · Focus mode · Full screen · Reset layout · Reset all layouts · Shortcuts |
| **Check** | Problems · Next problem · Previous problem · Issue revision… · Compare revisions… · Start commissioning |
| **Reports** | Every report in §3.4 (opens the Reports workspace on it) · Export all as PDF |
| **Tools** | Parts library… · Import parts · Macro library… · Sheets… · Simulation ▸ (live potentials, fault scenarios when they exist) · Python console (when WS-28 has one) |
| **Help** | Walk-through · Shortcuts · About KoperCAD |

`Navigate` commands (partner, all references, back, forward, previous page,
changes) sit in **View › Go to ▸**, because they are views of the same data.

### 5.3 Where the menus draw

- Window 1500 px or wider (effective): the menu names sit in the top bar after
  the mark, as words, left to right.
- Narrower: the mark opens one menu with the same eight names as cascading
  submenus. Nothing is missing, it is one click deeper.
- F10 or Alt+(first letter) opens the bar from the keyboard, arrows move,
  Esc closes. Menu items have no mnemonics of their own; a person who wants to
  type uses Ctrl+K.
- The first time a command runs from the menu while it has a key, the status
  bar says "Ctrl+Shift+F next time" once per command (remembered in settings).
  A teaching nudge in the place the person is looking, without a popup.

### 5.4 The mark

The mark keeps one job: the About line, Preferences and Quit on narrow
windows, and the menu bar's collapse target. It stops being a second menu.

## 6. Moving between workspaces

**Selection travels.** Selection is a set of model ids shared by all
workspaces, as the Cabinet and the Devices drawer already share it. On
switching:

| From → to | The selection is... |
| --- | --- |
| Schematic → Cabinet or 3D | A device selected in the drawing is selected on the plate and framed; an unplaced device selects its row in **Unplaced** and the status bar says "-K3 is not placed yet" |
| Cabinet ↔ 3D | The same ids; the camera frames them, with the 250 ms ease and not at all under reduced motion |
| Cabinet or 3D → Schematic | The device's main page and column, through the same `Place` the cross reference jump uses |
| any → Reports | The report that lists it (Device list by default), the row selected |
| any → Commissioning | The page of the selection, the item selected |

**History.** The jump history (Alt+Left, Alt+Right) records the workspace with
the place. Back from Cabinet returns to the Schematic page you came from, not
only to the previous page.

**Page and panel context.** The current panel (a location) is shared by
Cabinet and 3D. The current page is shared by Schematic and Commissioning.
Changing one in a workspace changes it for its partner, so the pair never
disagree about "which panel".

**Modes** keep their own state. Live stays on when the Schematic is left and
returns; Commissioning turns Live off because it recolours the drawing.

## 7. The status bar

22 px, `BG_1`, 11 pt text, `TEXT_MUTED`. Left to right:

| Slot | Shows | Replaces |
| --- | --- | --- |
| Message | The last status message for 5 s, then the workspace's idle hint ("Press Q or Space to place · ? for keys"). Errors stay until the next action. | The scattered toasts that carry a status; blocking errors still use a toast |
| Selection | "-K1 · Contactor · +CAB1" for one, "3 devices, 5 wires" for many, nothing for none | The "N selected" text on the canvas |
| Modes | A chip per active mode (Live, Changes, Marks hidden, Read-only), each a button that turns it off | New |
| Values | Cursor in mm, grid, snap, zoom, only in canvas workspaces; the camera distance in 3D | The floating HUD (`docs/DESIGN.md` §3), which goes away |
| Saved | Saving, Saved, Not saved yet | The top bar's save text (still the only save signal) |

The issue pill stays in the top bar: each count lives in exactly one place
(DESIGN.md §3). Status colours follow `ui-guidelines.md` and are never the
only signal. The bar is not clickable except the mode chips; the saved state shows the
project's file path in its tooltip.

## 8. The command surface (Ctrl+K)

It already lists every command grouped with shortcuts and disabled reasons.
What changes:

1. **Same data as the menus.** Rows come from the registry, so a command is
   listed in the same words in both. Each row shows its menu path in
   `TEXT_FAINT` on the right ("View › Workspace"), which teaches the menu.
2. **Workspace words.** `reports`, `3d`, `cabinet`, `commissioning` jump to
   the workspace, and `bom`, `wire list` and the other report names jump to
   the report. Dutch aliases follow the Workflow thread's table
   (`materiaallijst`, `draadlijst`, ...).
3. **Everything has a row**, including windows (`parts`, `macros`, `rules`).
   The test in §5.1 guarantees it.
4. **Context.** In a workspace, its own commands list first, then the rest.
   In Reports, `export` means the selected report.
5. **Recent** commands lead the empty field as today, and survive a restart.

## 9. Review checklist for features

A UI-facing feature PR answers these in its description, and the design
reviewer holds it to them:

1. **Home.** Which workspace, drawer tab, mode or window?
2. **Command.** Which registry rows, with menu and key?
3. **Entry points.** How does a person who has never heard of it find it (menu,
   status bar chip, empty-state text)?
4. **Selection.** What does it do when something is selected, and when the
   selection travels to it (§6)?
5. **Layout.** What is its default zone content, and does it remember its
   state (§4.1)?
6. **Keyboard.** Can it be used without the mouse?

## 10. Plan

Each step is a small pull request, a draft until its reviewers say READY, and
none of them is in a build until the owner asks for one. New code goes in new
files under `crates/novo-app/src/app/` to keep clear of the large files
other threads are editing (`app.rs` is over 2400 lines).

| Step | Pull request | Touches |
| --- | --- | --- |
| 1 | This document, with the five line edits in DESIGN.md §4, §6, §8 and §9 | docs only |
| 2 | **Workspace enum and state**: replaces `cabinet_open: bool`; the switcher in the top bar; Schematic and Cabinet; `Settings.workspaces` and the remembered layout; Reports, 3D and Commissioning chips disabled with their reason | `app/workspace.rs`, `settings.rs`, a few lines in `app.rs` |
| 3 | **One registry**: `menu` on `CommandInfo`, `Action` folded into `COMMANDS`, the invariant tests | `novo-workflow`, `actions.rs` |
| 4 | **Menu bar**, generated from the registry, collapsing into the mark | `app/menu_bar.rs` |
| 5 | **Status bar**: message, selection, mode chips, values; the HUD and the top-bar save text move | `app/status_bar.rs`, `app/workbench.rs` |
| 6 | **Panel registry**: drawer tabs per workspace, Alt+1 to Alt+9 for the tabs, F6 to cycle panels, Reset layout | `app/panels.rs`, `drawers.rs` |
| 7 | **Command surface**: menu path column, workspace and report words, Dutch aliases | `novo-workflow/palette.rs`, `command_field.rs` |
| 8 | **Reports workspace** hosting `novo-reports` | `app/reports_view.rs` |
| 9 | **3D workspace** hosting `novo-view3d` (when the renderer is on main) | `app/view3d.rs` |
| 10 | **Commissioning workspace** hosting `novo-commissioning` (when it is on main) | `app/commissioning.rs` |
| 11 | **Keys**: Ctrl+1 to Ctrl+5 for workspaces, drawer tabs move to Alt+digit (the Workflow thread agrees first, §11) | `keys.rs`, `actions.rs`, `RESERVED_SHORTCUTS` |

Steps 2 to 7 stand on their own: with only Schematic and Cabinet enabled they
already give the window a menu bar, a status bar and remembered panels. Steps
8 to 10 each switch on a chip that step 2 already placed.

Handoff for this thread: `handoff/workspaces/`.

## 11. Decisions that need the other threads

| Question | Who decides | Proposal |
| --- | --- | --- |
| Ctrl+3, Ctrl+4, Ctrl+5 move from drawer tabs to workspaces; tabs become Alt+1 to Alt+9 | Workflow | Yes: workspaces are the larger idea, and Alt+digit works for drawers with a different number of tabs per workspace. Ctrl+3/4/5 stay free of drawers from step 11; before that, nothing changes |
| DESIGN.md §8 and §9: menu bar, status bar and reports-as-commands reversed | Workflow (owns DESIGN.md), design reviewers | Yes, as in §1. This PR edits only the five DESIGN.md lines that say otherwise |
| 3D as a workspace rather than a Cabinet face | 3D threads (WS-29 and the look thread) | Yes, §3 |
| Right zone holds only Inspector and the Commissioning panel | Layout and branding | Yes |
| Status bar styling, workspace icons, menu typography | Layout and branding | Uses existing tokens; asks for four icons (Reports, Commissioning, 3D, Cabinet if not yet there) |
| `Action` folded into `COMMANDS` | WS-01 (app shell), WS-08 (keyboard) | One list in `novo-workflow`; `Action` becomes an id lookup |

## 12. Open questions

- **Panel search inside the menu bar.** Menus of 8 entries each are fine;
  Tools may grow. If a menu passes about 20 rows it gets a submenu per
  feature rather than a scroll.
- **Light and dark.** Only dark exists (`ui-guidelines.md`); nothing here
  depends on it.
- **A fifth zone for notes or chat with the AI assistant (WS-23).** Not
  designed here. The right zone rule means it would be a second Inspector tab,
  not a new zone.
