# KoperCAD 0.1.0 beta

KoperCAD is a local, offline electrical CAD program by KoperVonk. You draw IEC schematics with a
keyboard-first workflow, and the program keeps tags, potentials, cross references, terminal strips,
cables, parts and reports in step with the drawing. It then carries the design into the cabinet (2D and
3D) and out to the workshop. It installs per user, needs no administrator rights and works without a
network.

This is the first beta for test users. Start with [`docs/TESTER-README.md`](docs/TESTER-README.md).

Project files now end in `.kopercad`. Files from earlier builds (`.novo`) still open; use File > Save As
to keep a copy as `.kopercad`. An earlier build does not list `.kopercad` files in its Open dialog. The file
format itself did not change, so to open a new project in an old build, copy it to a `.novo` name first.

## What is in it

**Drawing**
- Keyboard-first editing: `Q` places (tap for search, hold for the wheel), `W` wires, `R` rotates, `Ctrl+D`
  duplicates and continues the pattern, `Tab` cycles connection points. `Ctrl+K` finds every command.
- Two wiring modes: Corners (the router finds the path) and Lines (you draw it), switched with `Shift+W`.
- IEC 60617 symbols and IEC 81346 tags (`=plant +location -device`), with an NFPA symbol set. Automatic tags,
  automatic potentials, interruption points, PE marking, feeds, blocks and macros.
- Wire and potential numbering with schemes, locks and conflict reports.
- Right-click menus, movable panels, saved window layouts, dark and light themes, density settings.

**Intelligence**
- Cross references between coils and contacts, with jumps.
- Terminal strips with connection screens (proposed terminals go on strips by kind: power, control, signals, protective earth), cables with cores and shields, PLC cards and I/O lists.
- Set the location of a whole selection at once (Devices tab, Inspector).
- Design checks with severity and location, a standards check (IEC 60204-1, NFPA 79, UL 508A rules),
  protection curves and selectivity, cable and wire sizing, functional-safety functions.
- Revisions: issue a revision (optionally locking the pages), freeze pages, compare two revisions, restore a page or the project. Scripts and imports that write into a locked page are refused until it is unlocked.

**Parts and reports**
- A part catalogue that you fill by importing maker data, with part assignment, quality checks and bulk edit.
- Generated pages that follow your edits: contents, bill of materials, device list, wire list, terminal and cable
  diagrams, I/O list. PDF export with bookmarks and links; Excel and CSV export.
- Bench output for the workshop: cut list, ferrule table, labels and an assembly checklist. Reports and
  checklists can be switched to Dutch.

**Cabinet**
- Cabinet layout with mounting plate, rails and ducts, "place all", clash checks and enclosure sizing, door
  parts and drill rows.
- Wire routing through the ducts gives wire lengths.
- 3D view of the cabinet with orbit, measure, align, section plane and walls, roof, floor and extra plates.
- Heat and airflow model with hot spots, compare and move-and-solve. See "Numbers" below. Wire ducts now stand as high as their cross-section and are air in the heat slice, so heat results for projects with ducts differ from earlier builds.

**Simulation and commissioning**
- Live view (`Shift+L`): hold a pushbutton, latch a contactor, watch coils, contacts, timers, lamps and motors.
  Motor start estimate and fault scenarios.
- Commissioning mode with a check list, a punch list with photos and signatures, and Field mode that marks
  redlines on the drawing and the cabinet.

**Learning and help**
- Learn: 33 short lessons that run inside the program and check your work (10 finished, the rest marked
  "Early access"). An example project and a fast-keys walk-through on the start screen.

**Import and scripting**
- DXF export of pages and plates, DXF import as a reference underlay, STEP import for parts.
- `kopercad-cli` and a Python module for scripting and checks.

## Numbers: what is exact and what is a model

KoperCAD tells you which is which. Please read the labels.

| Number | Status |
| --- | --- |
| Wire lengths before routing | Placeholder, shown with `~` |
| Wire lengths after "Route all" | Geometric, from the layout and duct model |
| Cable ampacity, derating, voltage drop | Planning values; the tables are not verified against the standard |
| Heat, airflow, device temperatures | A physical model with a stated accuracy (`docs/thermal-validation.md`); the 3D airflow result is marked "not validated" |
| PFHd, performance level | Closed-form estimate, "model, not exact"; not a SISTEMA replacement |
| Motor start current | Estimate |
| Standards checks | A subset of the rules, not a certification |
| Weights and prices | Only from part data that has them |

## Known limitations

- Windows 10 and 11, 64 bit only. Most testing so far was on automated builds; real-PC testing is what this beta is for.
- The dark theme is the supported one; some screens are not fully themed in light.
- Report language is English; Dutch covers the assembly checklist and the reports, not the whole program.
- The parts library holds only what you import; no maker catalogues or EPLAN data are bundled.
- Not in this release: EPLAN project import or export, PLC program export, symbol editor, label printer
  drivers (label CSV and PDF exist), several people editing at once, automatic updates, an AI assistant.
- 3D view: the section plane is a slider (no mouse drag), heat arrows show no value on hover. The autosave interval
  is fixed, there is no custom symbol folder, terminal jumper numbers are automatic only.
- STEP import approximates some curved surfaces.
- Big projects (about 1,500 symbols and wires or more): the design check runs in the background, so the issue counts and the Issues list can update a moment after an edit (a small pulsing dot by the issue counts shows while they catch up). Reports and exports always wait for the current answer. Measured on one synthetic 600-page project without a graphics card, a still page took about 8 ms of program time per frame; that is a measurement on one machine, not a promise.
- The program is not code-signed, so Windows SmartScreen warns on first start: choose "More info", "Run anyway".

## Install

- Installer: `KoperCAD-0.1.0-setup.exe` installs for the current user in `%LOCALAPPDATA%\Programs\KoperCAD`.
- Portable: unzip `KoperCAD-0.1.0-portable.zip` anywhere and run `run.cmd`.
- Projects live in `Documents\KoperCAD`, settings in `%APPDATA%\KoperCAD`. Uninstalling keeps both.
- Check the download against `SHA256SUMS` on the release page.

## Report a problem

Use Help > Copy problem report and paste it into your message, with what you did, what happened, and the
project file if you can. After a crash, KoperCAD offers the crash report on the next start; Help > Open log folder
opens `%LOCALAPPDATA%\KoperCAD\logs`. Details are in the tester README.

## Licences

KoperCAD is open source under either the MIT licence ([`LICENSE-MIT`](LICENSE-MIT)) or the Apache-2.0 licence
([`LICENSE-APACHE`](LICENSE-APACHE)), at your option. Copyright 2026 KoperVonk. Third-party software is listed in
[`THIRD_PARTY_NOTICES.txt`](THIRD_PARTY_NOTICES.txt).
