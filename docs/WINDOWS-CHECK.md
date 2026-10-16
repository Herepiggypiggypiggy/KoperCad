# Windows hands-on check

What only a person on a Windows PC can confirm. Nothing here has been run on Windows by the
people or tools that wrote it. Do the sections for the build you were given; each line says what
you should see. Anything that differs: note the step and what happened.

No step needs admin rights. If one asks for it, stop and say so.

## 1. Start the new build

1. Double-click the desktop shortcut **KoperCAD (latest)**. KoperCAD opens (title "KoperCAD").
2. Help, About: shows "by KoperVonk" and the build time.
3. If the shortcut did nothing, run `scripts\install-desktop-shortcut.ps1` once and try again.
   (That refreshes the launcher copy; an older copy still works because the build also ships
   `novoelectric.exe`.)
4. In `%LOCALAPPDATA%\NovoElectric\experimental\current` there is both `kopercad.exe` and
   `novoelectric.exe`. Task Manager shows `kopercad.exe` while the app runs.

## 2. Data move (first start after the rename)

1. `%APPDATA%\KoperCAD` exists and `%APPDATA%\NovoElectric` is gone (it was renamed).
2. Window size, panels and recent files are as you left them.
3. Preferences, parts catalog: your own parts are still there (`%APPDATA%\KoperCAD\parts.json`).
4. If you already had projects in `Documents\NovoElectric`, a new project's first save still goes
   there. With no old folder, it goes to `Documents\KoperCAD`.
5. Close and start again: nothing is moved or reset a second time.

## 3. Build here (only if you use it)

1. Run the shortcut **KoperCAD (build here)**. The first run downloads Rust and a portable
   compiler and takes a long while. A warning "download is unchecked" is harmless; "failed its
   SHA-256 check" is not: report it.
2. It ends with "Done: main at <sha>" and starts KoperCAD.

## 4. Installer upgrade (only if you use the installer)

1. Run the new `KoperCAD-<version>-setup.exe` over an older install. No admin prompt appears.
2. After it: one Start menu entry and one desktop icon (if chosen), double-clicking a `.novo` file
   opens KoperCAD, and no `novoelectric.exe` is left in `%LOCALAPPDATA%\Programs\KoperCAD`.

## 5. Editor points to confirm

- **Wheel (points 14 and 16).** Hold Q: the wheel opens, no `qqqq` appears in the search. Move
  the pointer across a wedge without stopping: the outer ring does not jump to it. Rest on a
  wedge for a moment: the outer ring follows. Choosing in the first ring does not place anything
  until you choose in the second ring.
- **Moving preference.** Preferences, "Moving a symbol": "Leaves its wires" is the default. Drag
  a symbol: it moves alone and its wires stay. Switch to "Takes its wires along": a plain drag now
  carries the wires. Shift always does the other one.
- **Shift+M (needs a build that includes #294; skip it otherwise).** Select a symbol and press
  M: it moves as the Moving preference says. Shift+M always carries the wires, and keeps
  carrying them after you release Shift. Esc cancels and puts it back.
- **Shift+L, live potentials.** Shift+L turns the live view on: a badge sits on each switched
  pole, click it to close or open the pole. Lit wires show the potential colour, unreached
  wires are dimmed, a net where two potentials meet is red. Hover a wire for a tip about what it
  carries. Coils get a badge too and timers count down. Shift+L (or the close button) leaves the
  view and the drawing is unchanged.
- **Example and fast-keys walk-through (needs a build that includes both).** On the start panel
  click Open example: a motor starter opens over two pages plus generated pages. Close it with
  Ctrl+W without touching it: nothing appears under Unsaved. Click Try the fast keys: a practice
  page with a motor and three terminals opens and a card at the bottom right lists seven steps.
  Press W, C, Ctrl+D, Ctrl+K, hold Q, Shift+L: each step gets a tick. Skip closes the card and it
  does not return by itself; Ctrl+K, "Fast keys walk-through" brings it back. The card must not
  cover the tool dock.

## 6. Cabinet table, DXF export, recovery

- **Cabinet table (needs #403).** Open the cabinet view of a panel with a few devices. Click the
  table button in the cabinet toolbar (tip "Devices as a table (Ctrl+T)"), or press Ctrl+T: a
  table replaces the plate with the columns Device, Size (mm), X, Y, Turned, Rail, Problems.
  Ctrl+T again returns to the plate. Move a device so it overlaps another: on the plate its mark
  shows a hover text such as "Overlaps -Q2", and the same words appear in the table's Problems
  column.
- **Export DXF (needs #385).** Open a project with two or more pages and save it. File, Export
  DXF, This page: a `.dxf` for the current page appears next to the project file and a message
  says so. Export again: the old file is not overwritten silently, you get a numbered copy or a
  note that it was replaced. File, Export DXF, All pages: a folder `<project> DXF` with one
  `.dxf` per page. Open one in any DXF viewer: the symbols and wires are on the page. With
  nothing open the menu is greyed out.
- **DXF background (needs #385).** File, DXF background, Import DXF: pick a `.dxf`. It shows
  grey under the drawing, cannot be selected or moved, and is saved with the project.
- **Recovery after a crash.** Open a project, draw a wire (do not save), then end KoperCAD in
  Task Manager (End task). Start KoperCAD again and open the same project: a dialog "Restore
  unsaved work?" names the project and when it was autosaved. Escape and a click outside do
  nothing; only Restore or Discard answer it. Restore brings the wire back; Discard leaves the
  last saved version. After a normal close there is no dialog at the next start.

Send the result to the thread you got the build from, or write it next to the build number.
