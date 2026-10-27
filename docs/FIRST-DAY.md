# First day at work with KoperCAD

For the person who will use the program at work tomorrow. Plain steps, about 20 minutes. No admin rights are needed
at any point; if something asks for them, stop and say so.

## 1. Before you start: protect your work

- **Copy your real projects somewhere safe first** (a USB stick or the network share). Work on the copy until you trust
  the build. A newer build opens older files and writes `<file>.v<old>.bak` beside it before the first change; an older
  build refuses a newer file without touching it.
- Do not use a draft release candidate on the only copy of a customer project.

## 2. Install (two ways, pick one)

**Installer (recommended).** Download `KoperCAD-<version>-setup.exe` and `SHA256SUMS` from the release page. In
PowerShell: `Get-FileHash .\KoperCAD-<version>-setup.exe -Algorithm SHA256` and compare with the line in `SHA256SUMS`.
Run the setup. Windows may say "Windows protected your PC" because the file is not code-signed: choose **More info**,
then **Run anyway**. No admin prompt should appear. It installs under `%LOCALAPPDATA%\Programs\KoperCAD`.
Tick the desktop icon if you want one.

**Portable zip (no installer at all).** Download `KoperCAD-<version>-portable.zip`, unzip it anywhere you may write
(for example `Documents`), double-click `run.cmd`. Settings and logs stay in the `data` folder beside it.

If the window stays black or the program does not start, use the Start menu entry **KoperCAD (safe graphics)**
(portable: `run-safe-graphics.cmd`).

## 3. The first ten minutes

1. Start KoperCAD. Help > About shows the version, maker (KoperVonk) and your graphics adapter.
2. **Open one of your projects** (a copy): File > Open, or double-click the `.kopercad` or older `.novo` file.
   Look through the pages. Press **Ctrl+Z** a few times and **Ctrl+Shift+Z** to see undo works on your project.
3. **Save as** a new name (File > Save as). Close and reopen it: it should look the same.
4. **Print**: File > Export, the PDF of the whole set (use Print preview first). Open the PDF and check the title block and a page you know well.
5. **Parts import** (if you have a price list or catalogue): File > Import parts > Import parts from a table (CSV or
   Excel). Read the mapping step and the import report before accepting it.
6. Press **Ctrl+K** and type what you want to do ("wire list", "bench", "kitting", "test every wire"). Letter keys work
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

## 5. What to treat as a model, not a fact

- Heat, airflow and cooling results are models, not measurements. Check a critical enclosure with the maker's tool.
- EMC is a rule-based design aid. It is never a compliance statement.
- Wire sizes, ampacity, colours, test limits and kitting lengths are planning values. The standard clause numbers
  (IEC 60204-1) are not checked against a licensed copy.
- Lengths marked **~** are estimates until the cabinet is laid out and routed.
- Ask Claude (optional, needs your own key) has only been tested with a scripted model.

The full manual is written when you open Help > User manual (F1; also `docs/manual/`). The honest list of what is unproven is
in the release notes of the build you installed.
