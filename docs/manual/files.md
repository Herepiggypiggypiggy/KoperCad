# Files, formats and backups

## Where things are

Everything is in your own user profile and nothing needs administrator rights.

| What | Where |
| --- | --- |
| Projects | `Documents\KoperCAD` (or `Documents\NovoElectric` if you used an earlier build) |
| Settings, parts catalogue, key map, templates | `%APPDATA%\KoperCAD` |
| Recovery copies (autosave) | `%LOCALAPPDATA%\KoperCAD\recovery` |
| Log and crash reports | `%LOCALAPPDATA%\KoperCAD\logs` (Help > Open log folder) |
| This manual, written when you open it | `%LOCALAPPDATA%\KoperCAD\manual` |

## File types

| Extension | What it is |
| --- | --- |
| `.kopercad` | A project: one SQLite file with pages, devices, wires, parts, revisions and commissioning data. Copy it, mail it, back it up like any file. |
| `.novo` | A project from before the rename. Still opens. |
| `.kcfield` | A field file: commissioning results and Build mode ticks, exchanged between desk and tablet. It holds no drawing. |
| `.pdf`, `.xlsx`, `.csv` | Exports: drawings, reports, label and bench data |
| `.dxf`, `.step` | DXF: imported as a background and exported per page or as a wiring plan. STEP: imported for part shapes and exported for the cabinet. |

## Compatibility promises

- A newer build opens older files. Before the first write the old file is copied to `<file>.v<old>.bak`.
- An older build **refuses** a file from a newer build, in words, without touching it.
- A save is designed not to leave half a file: after a crash or power cut you get the previous save or the new one. A test that kills the program at staggered moments while it saves checks this (`compat_v0_1`); it has been run on Linux only.
- A write-protected file opens read-only (title says so, Save is disabled; use Save as).
- A damaged file is never edited. The program salvages what it can into `<name>.recovered.kopercad` beside it, opens that, and shows "Recovered copy: check every page" until you confirm.

The full text is in `docs/FILE-COMPATIBILITY.md`.

## Import and export

File > Import: DXF background, DXF drawing, AutomationML, parts tables and folders, PLC tags. File > Export: PDF (whole set, one page, variant), DXF, STEP of the cabinet, reports, handover package, PLC lists, print preview.

## Command line and Python

`kopercad-cli` reads a project without opening the app (it never writes to the project): checks, reports, PDF export, comparison of revisions. Run `kopercad-cli --help`. `kopercad-cli manual <folder>` writes this manual to a folder. The Python module `kopercad` scripts the same functions.
