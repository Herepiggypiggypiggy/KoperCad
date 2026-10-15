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
- **Shift+M.** With the pointer over a symbol, M moves it as the preference says; Shift+M does
  the opposite (picks it up with its wires if the preference is "Leaves its wires"). Esc cancels
  and puts it back.
- **Shift+L, live potentials.** Shift+L turns the live view on: a badge sits on each switched
  pole, click it to close or open the pole. Lit wires show the potential colour, unreached
  wires are dimmed, a net where two potentials meet is red. Shift+L (or the close button) leaves
  the view and the drawing is unchanged.

Send the result to the thread you got the build from, or write it next to the build number.
