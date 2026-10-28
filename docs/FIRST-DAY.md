# First day at work with KoperCAD

For the person who will use the program at work tomorrow. Plain steps, about 20 minutes. No admin rights are needed
at any point; if something asks for them, stop and say so.

## 1. Before you start: protect your work

- **Copy your real projects somewhere safe first** (a USB stick or the network share). Work on the copy until you trust
  the build. A newer build opens older files and writes `<file>.v<old>.bak` beside it before the first change; an older
  build refuses a newer file without touching it.
- A release candidate is a test build: never let it be the only place a customer project lives.
- **Work on a local disk** (for example Documents on the PC), not on a network drive or a synced folder. A project is a SQLite file and a share may not honour its locks; this was not tested. Copy to the share at the end of the day.

## 2. Install (two ways, pick one)

**Where the files are.** The work build is a draft release on GitHub, so it is visible only when you are signed in to the
KoperCAD repository: open Releases and pick the version you were told (for example `v0.1.0-rc9`). Download them **at home, the evening before**: a company proxy may block them. Its files are
`KoperCAD-<version>-setup.exe`, `KoperCAD-<version>-portable.zip` and `SHA256SUMS`. If you cannot see the draft, ask for
the three files to be sent to you another way. The public `experimental` pre-release holds a different, single zip
(`kopercad.exe` only): do not use it for work.

**Installer (recommended).** Download the setup and `SHA256SUMS`. In
PowerShell: `Get-FileHash .\KoperCAD-<version>-setup.exe -Algorithm SHA256` and compare with the line in `SHA256SUMS`.
Every release candidate's installer has the same name (`KoperCAD-0.1.0-setup.exe`), so tell them apart by the hash
and by Help > About after installing. Run the setup. Windows may say "Windows protected your PC" because the file is not code-signed: choose **More info**,
then **Run anyway**. No admin prompt should appear. It installs under `%LOCALAPPDATA%\Programs\KoperCAD`.
Tick the desktop icon if you want one.

**Portable zip (no installer at all).** Download `KoperCAD-<version>-portable.zip`, right-click it, Properties, tick **Unblock**, OK (or `Unblock-File` in PowerShell), then unzip it anywhere you may write
(for example `Documents`), double-click `run.cmd`. Settings and logs stay in the `data` folder beside it.

To update later, **close KoperCAD first** (save your work), then run the newer setup. If a company policy blocks the installer, try the
portable zip.

Pick one of the two, installed copy or portable zip: they keep separate settings (installed: `%LOCALAPPDATA%` and `%APPDATA%`;
portable: the `data` folder). Save projects into an explicit folder in Documents.

If the window stays black or the program does not start, use the Start menu entry **KoperCAD (safe graphics)**
(portable: `run-safe-graphics.cmd`).

## 3. The first ten minutes

1. Start KoperCAD. Help > About shows the version, maker (KoperVonk) and your graphics adapter.
2. **Open one of your projects** (a copy): File > Open (with the installer you can also double-click the `.kopercad` or older `.novo` file).
   Look through the pages. Press **Ctrl+Z** a few times and **Ctrl+Shift+Z** to see undo works on your project.
3. **Save as** a new name (File > Save as). Close and reopen it: it should look the same.
4. **Print**: File > Export, the PDF of the whole set (use Print preview first). Open the PDF and check the title block and a page you know well.
5. **Parts import** (if you have a price list or catalogue): File > Import parts > Import parts from a table (CSV or
   Excel). Read the mapping step and the import report before accepting it.
6. Press **Ctrl+K** and type what you want to do ("wire list", "bench", "kitting", "commissioning"; the wire test is the fourth tab of Commissioning). Letter keys work
   on the drawing only; click it first. A grey command tells you why when you hover.

## 4. If something goes wrong

| You see | Do this |
| --- | --- |
| Program closed or crashed | Start it again. It offers to restore unsaved work. Your file only changes at Ctrl+S. |
| A file will not open | It is salvaged into `<name>.recovered.kopercad` beside it and opened for you to check every page. Your original is untouched. |
| Something looks wrong or is cut off | Maximise the window (1366 x 768 is the smallest size tested). |
| Black window | Start **KoperCAD (safe graphics)**. |
| You want to tell us | Help > **Copy problem report**, paste it with what you did and what you expected. Nothing is sent anywhere. |

Your work is in `Documents\KoperCAD`, settings in `%APPDATA%\KoperCAD`, recovery copies in
`%LOCALAPPDATA%\KoperCAD\recovery`, logs in `%LOCALAPPDATA%\KoperCAD\logs` (Help > Open log folder).

## 4b. Small known problems (none loses work)

- Unnumbered wires show as "Wire wire" in the Commissioning Test tab.
- In the New cabinet dialog, Ctrl+K and typed words go into the Name field.
- Closing an edited example project gives no prompt: a draft is kept under Unsaved on the start screen (named "Untitled", without undo history).
- A price written "1.500" or "1,500" is read as 1.5 with a warning.
- Also in rc9, fixed in the later rc10 draft: Delete with the pointer off the page is ignored without a message; F2 rename appends to the old tag instead of replacing it; Ctrl+N from 3D stays on the greyed 3D tab; Ctrl+K "wire numbers" opens a lesson first; the Commissioning side panel clips its counters; an empty cabinet shows hot-spot cards.

## 5. What to treat as a model, not a fact

- Heat, airflow and cooling results are models, not measurements. Check a critical enclosure with the maker's tool.
- EMC is a rule-based design aid. It is never a compliance statement.
- Wire sizes, ampacity, colours, test limits and kitting lengths are planning values. The standard clause numbers
  (IEC 60204-1) are not checked against a licensed copy.
- Lengths marked **~** are estimates until the cabinet is laid out and routed.
- Ask Claude (optional, needs your own key) has only been tested with a scripted model.

The full manual is written when you open Help > User manual (F1; also `docs/manual/`). The honest list of what is unproven is
in the release notes of the build you installed.
