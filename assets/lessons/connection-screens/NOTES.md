# connection-screens: notes for the reviewer

Teaches: the generated "Terminal diagram" page of a strip (what each column and the jumper bar show), that it follows the
drawing live (cables accepted in the Cables drawer, a terminal renamed). 11 steps, about 9 minutes, `status` unchanged (`draft`),
`version` 2 (wording changed).
Project: terminals.rs `connection_screens_start`: strip +CAB-X1 with 8 terminals (1 2 3 PE 4 5 6 7), wire numbers 101.. / 201..,
terminals 5 and 7 bridged, pump cable -W1 (4 wires), two valve cables still proposed, generated pages 0 10 20 30 40 50.

## Replay

crates/novo-app/src/app/lesson_verify_terminals.rs `connection_screens_steps_work_in_the_window` starts the shipped lesson and
does every step through the window: the painted "Pages" button and the painted outline row, Ctrl+4 / Ctrl+3, a click on the
painted "Accept 2 proposed" button, a click on the cell "4" under the head "Terminal", a real double click on the terminal name,
Ctrl+A, typed text and Enter, Alt+Left. The generated page cannot be painted headless (no graphics adapter), so its content is
read from the `SheetContent` the canvas was given (texts and lines with their millimetre positions): heads, cabinet side
(x 14..128), blocks (x 156..178), the jumper bar, the Cable column (x 320) and the row "11" at the place where "4" stood.

## Verified path per step (paths under crates/)

1. goal: reading.
2. open-diagram: outline "Pages" flyout (app.rs) or pinned panel; row "40  Terminal diagram · +CAB-X1" (app/backend.rs `page_rows`); check `page`.
3. read-diagram: reading; sheet = novo-reports strip_diagram.rs `strip_sheet`, shown by app/reports.rs.
4. open-cables: Ctrl+4 (keymap.rs) -> workbench.rs; check `command view.cables_drawer`.
5. accept-cables: button "Accept 2 proposed" (app/drawers.rs) -> `accept_cable_proposals`; check `cable -W2 cores >= 2`; toast "Accepted 2 cables: -W2 3x1.5, -W3 3x1.5".
6. see-follow: reading; the sheet shows -W2 and -W3 twice each in the Cable column (LiveReports regenerates by dependency).
7. open-terminals: Ctrl+3 switches the tab; check `command view.terminals_drawer`.
8. show-terminal: click the name cell "4" (app/terminals.rs `strip_table`) -> drawers.rs `DrawerAction::Go` -> workbench.rs `jump` (records history); the terminal is selected.
9. rename: double click the name left of the terminal (novo-editor view.rs `LabelKind::TerminalName`) -> edit.rs `rename_label`; check `terminal -X1 / 11`.
10. back-to-diagram: Alt+Left = nav.back -> xref.rs `go_history`; check `page`.
11. done: reading, ends with what to look for in finished.json.

## Instructions the program REFUSED or got wrong, and the fix
- back-to-diagram ticked the moment it opened: its `point` area was on the diagram page, and a target on another page makes the runner switch the view when the step opens, so the `page` check was already true. The `point` is removed (a step whose goal is to change page must not point at the target page).
- rename: typing "11" into the box without selecting first gives "411" (the box keeps the old name and the cursor goes to its end) and the check never ticked. The text now says to select it all with Ctrl+A.
- The text said `{key:view.pin_outline}` keeps the outline open; that command has no key and renders "(no key)". Replaced by the click path (Pages button, then the row).
- `{ui:drawer.*.tab}` rendered as the word "tab"; replaced by plain words. `opened = drawer.*` could never tick (no anchors); replaced by `command view.*_drawer`.

## GAPs closed since the first version
- check kind for a terminal (`terminal = { strip, name }`) exists and is used in rename.
- solve op `cable` exists; accept-cables uses it instead of the invented `cables.accept_proposed`.

## GAPs still open
- No solve op `rename` (terminal "-X1:4" to "11"): the rename step's `solve` is a placeholder `select` (`# GAP solve`).
- The drawer button "Accept 2 proposed" still has no command id (the `cable` ops build the same two cables, so Show-me runs, but the button is the real control).
- F2 still does not rename a terminal (view.rs F2 handler looks for a device only); the lesson names the double click.
- Anchors missing for the outline button, the drawers and their tabs, the table cells: no pointers on open-diagram, open-cables, accept-cables, open-terminals.
- A terminal name (and an interruption name) in the rename box is not preselected; the person needs Ctrl+A.
- The Terminals drawer shows 7 of the 8 rows without scrolling at the 1600x900 test size.

## Pending how

Paste as `how = [...]` / `how_nl = [...]` when the framework reads the field.

### open-diagram
- EN: ["If the page list is not on the left, click **Pages** at the top left", "Click the row **40  Terminal diagram · +CAB-X1**"]
- NL: ["Staat de paginalijst niet links, klik dan linksboven op **Pages**", "Klik op de rij **40  Terminal diagram · +CAB-X1**"]

### open-cables
- EN: ["Press {key:view.cables_drawer}, or click the **Cables** tab at the bottom of the window"]
- NL: ["Druk op {key:view.cables_drawer}, of klik onderin op het tabblad **Cables**"]

### accept-cables
- EN: ["In the Cables drawer click **Accept 2 proposed** (top right of the drawer)"]
- NL: ["Klik in de lade Cables op **Accept 2 proposed** (rechtsboven in de lade)"]

### open-terminals
- EN: ["Press {key:view.terminals_drawer}, or click the **Terminals** tab at the bottom of the window"]
- NL: ["Druk op {key:view.terminals_drawer}, of klik onderin op het tabblad **Terminals**"]

### show-terminal
- EN: ["In the Terminals drawer click the number **4** in the first column (**Terminal**)"]
- NL: ["Klik in de lade Terminals op het nummer **4** in de eerste kolom (**Terminal**)"]

### rename
- EN: ["On the drawing, double-click the number **4** just left of the selected terminal", "Press Ctrl+A to select the old number in the box", "Type **11**", "Press Enter"]
- NL: ["Dubbelklik op de tekening op het nummer **4** links van de geselecteerde klem", "Druk op Ctrl+A om het oude nummer in het vak te selecteren", "Typ **11**", "Druk op Enter"]

### back-to-diagram
- EN: ["Press {key:nav.back}"]
- NL: ["Druk op {key:nav.back}"]

Reading steps (goal, read-diagram, see-follow, done) need no `how`.
