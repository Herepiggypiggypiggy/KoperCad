# Clean-profile Windows installer check

The one thing no automated check has seen: the installer and first start on a Windows PC that
has never had KoperCAD. The build server proves the installer compiles, installs per user without
admin rights and that the app starts; it cannot show the wizard pages, the uninstall question,
the graphics adapter, HiDPI or double-click opening. Do this once per release candidate and send
the result to the thread you got the build from.

Use a fresh Windows user account (Settings, Accounts, Other users) or a PC that never had
KoperCAD, NovoElectric or a `kopercad` folder. A standard user is fine; no step needs admin
rights. If one asks for it, stop and say so. Download `KoperCAD-<version>-setup.exe` and
`SHA256SUMS` from the release.

## A. Before installing

1. In PowerShell: `Get-FileHash .\KoperCAD-<version>-setup.exe -Algorithm SHA256`. The value
   equals the line in `SHA256SUMS`.
2. Right-click the file, Properties: it is not code-signed, so SmartScreen may warn on first run
   ("More info", then "Run anyway"). Note that it appears.
3. Confirm these do not exist yet: `%LOCALAPPDATA%\Programs\KoperCAD`, `%LOCALAPPDATA%\KoperCAD`,
   `%APPDATA%\KoperCAD`.

## B. Install

1. Run the setup. **No UAC (admin) prompt appears.** Note any that does.
2. The wizard offers a per-user location under `%LOCALAPPDATA%\Programs\KoperCAD`, the task
   "Open .kopercad (and older .novo) project files with KoperCAD" (ticked) and a desktop icon
   (unticked). Tick the desktop icon once.
3. Finish with "Launch KoperCAD". The window opens with title "KoperCAD" and the start panel.
4. Start menu has **KoperCAD** and **KoperCAD (safe graphics)**; the desktop has one icon. Not
   present: any `novoelectric` entry.
5. Help, About: maker KoperVonk, licence, the graphics adapter line names your real GPU (not a
   software renderer); "Copy version info" puts text on the clipboard; "Open log folder" opens
   `%LOCALAPPDATA%\KoperCAD` or a folder under it.
6. Note the window at your display scale (100%, and 150% or 200% if you can change it): text and
   icons sharp, nothing cut off, no console window flashing at start.

## C. First use

1. Open example from the start panel, then close with Ctrl+W: nothing appears under Unsaved.
2. New project, draw one wire, Save as `test.kopercad` in Documents. Close KoperCAD.
3. Double-click `test.kopercad` in Explorer: KoperCAD opens that project. Right-click the file:
   the icon and "KoperCAD project" type show.
4. Rename a copy to `test.novo` and double-click it: it opens too.
5. Start from **KoperCAD (safe graphics)**: it opens (software renderer). Note it.
6. Crash recovery: draw a wire, do not save, End task in Task Manager, start again and open the
   project. "Restore unsaved work?" appears; Restore brings the wire back.

## D. Update over a running copy

1. With KoperCAD running, run a newer setup over it. Note what the installer says about the
   running program (ask to close it, or continue).
2. Afterwards one Start menu entry, one desktop icon, the project still opens, settings and
   recent files are kept.

## E. Uninstall

1. Settings, Apps, KoperCAD, Uninstall (or the Start menu entry).
2. A question "Also remove KoperCAD data from this PC?" appears. The default button is **No**.
   Answer No first: `%LOCALAPPDATA%\KoperCAD` and `%APPDATA%\KoperCAD` stay, the program folder
   and shortcuts are gone, `test.kopercad` in Documents is untouched.
3. Reinstall: your settings are back. Uninstall again and answer **Yes**: both data folders are
   gone, `test.kopercad` is still there.
4. Check that the `.kopercad` and `.novo` associations are gone (double-click the file: Windows
   asks which app to use).

## F. Portable zip (optional)

1. Unzip anywhere (for example the Desktop) and double-click `run.cmd`; or
   `run-safe-graphics.cmd`. A file `kopercad.portable` next to the exe keeps settings, logs and
   recovery files in the `data` folder beside it, and nothing appears under `%APPDATA%\KoperCAD`.
2. `kopercad-cli.exe --help` runs in a terminal.

## Report

Per section: OK, or the step number and what happened (screenshot of any dialog). Also give the
Windows version, display scale and GPU. Anything in B.1 (an admin prompt), E.2 (the default not
being No, or a project file removed) or C.6 (lost work) is a blocker for the release.
