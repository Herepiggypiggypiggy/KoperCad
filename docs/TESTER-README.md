KoperCAD 0.1.0 beta: read this first
====================================

KoperCAD is an electrical CAD program by KoperVonk. You draw schematics with the
keyboard, and the program keeps the tags, wire numbers, cross references,
terminal strips, cables, parts list and reports in step with the drawing. It
also lays out the cabinet (in 2D and 3D), checks the design, simulates the
circuit and prints a bench output for the workshop.

This is a beta for test users. It works offline, it needs no account, and it
never needs administrator rights. Thank you for trying it.


1. Start it
-----------

You have one of two things:

* The installer, KoperCAD-0.1.0-setup.exe. Double-click it. It installs for you
  only (no administrator prompt). Start KoperCAD from the Start menu.
* The portable zip, KoperCAD-0.1.0-portable.zip. Unzip it anywhere you can write
  to, for example your Documents folder, and double-click run.cmd (or
  kopercad.exe directly). Nothing is installed.

Windows may show a blue "Windows protected your PC" box the first time, because
the program is not code-signed yet. Choose "More info", then "Run anyway".

To check that your download is complete, compare it with the SHA256SUMS file on
the release page. In PowerShell:

    Get-FileHash .\KoperCAD-0.1.0-portable.zip -Algorithm SHA256

It runs on Windows 10 and 11, 64 bit. A laptop screen of 1366 x 768 is fine. If
the window does not fit your screen, double-click its title bar to maximise it.

If it will not start, or shows a start-up message, see "If something goes wrong"
below. Do not try to run it as administrator; it will not help.


2. Your first ten minutes
-------------------------

1. On the start screen click **Open example**. A motor starter opens over two
   pages. Nothing you do to it can damage a file: it is an untitled copy.
2. Press **Ctrl+K** and type a few letters of anything ("cabinet", "export",
   "wire numbers"). This search finds every command and tells you which menu it
   lives in. If you only remember one key, remember this one.
3. Click **Try the fast keys** (start screen) or use Help > Fast keys walk-through.
   A small card lists the keys that make drawing quick, and ticks each one when
   you really use it:
   * **Q** place a symbol (tap it for the search, hold it for the wheel)
   * **W** draw a wire (Shift+W switches between Corners and Lines)
   * **R** rotate, **Ctrl+D** duplicate and carry on the pattern
   * **Tab** next connection point while wiring
4. Open Help > **Learn**. There are short lessons that run inside the program
   (motor starter, wire numbers, terminal strips, cabinet layout and more). The
   lesson checks your work as you go. Lessons marked "Early access" work but
   are less polished.
5. Switch workspace with the buttons in the top bar (Schematic, Cabinet, 3D,
   Reports, Commissioning), or Ctrl+1 for Schematic and Ctrl+2 for Cabinet. Right-click
   anywhere for the commands that fit what you clicked.

Then try your own circuit: File > New project, press Q, place a breaker, a
contactor, an overload and a motor, press W to wire them, and watch the tags and
wire numbers appear.

Good things to test (tell us how they felt, not only whether they broke):

* Draw a motor starter from scratch. Was the keyboard faster than the mouse?
* Assign parts, then open Reports and look at the bill of materials.
* Save, close, open again. Is everything exactly as you left it?
* Cabinet workspace: size the enclosure, "place all", look at the 3D view.
* Press Shift+L for the live view: hold Start, see the contactor pull in.
* Export a PDF of the drawing and a bench output (cut list, ferrules, labels).


3. Where your files are
-----------------------

Everything is in your own user profile. You can look at it all in File Explorer
(type the path into the address bar).

* Projects: Documents\KoperCAD (or Documents\NovoElectric if you used an earlier
  test build; the program keeps using that folder). A project is one file ending
  in .kopercad (older projects: .novo, which still open). Copy it, mail it, back it up like any file.
* Settings, your parts catalogue, your key bindings and your own templates:
  %APPDATA%\KoperCAD
* Recovery files (the autosave of a project while you work on it):
  %LOCALAPPDATA%\KoperCAD\recovery
* Problem log and crash reports (the program writes a crash report there): %LOCALAPPDATA%\KoperCAD\logs
* Projects you have not saved yet: %LOCALAPPDATA%\NovoElectric\untitled (the old name is kept)
* The installer puts the program in %LOCALAPPDATA%\Programs\KoperCAD.

KoperCAD never writes to Program Files or to the registry outside your own user
settings.

Autosave: while a project is open the program saves a recovery copy regularly.
If it stops unexpectedly (a crash, a power cut), start it again and it offers the
recovered work. A clean close removes the recovery file. Your .kopercad file is only
changed when you press Ctrl+S.

Uninstalling (Windows Settings > Apps) removes the program and leaves your
projects and settings alone. To start completely fresh without touching your real settings, run
kopercad.exe --profile-dir C:\Users\you\kopercad-test (a new empty folder; delete it to start
over). Everything the program keeps then goes into that folder.


4. If something goes wrong
--------------------------

A short report that we can act on is worth more than a long one. Please:

1. Choose Help > **Copy problem report**. It copies the version, build, your system, the
   graphics card and the last lines of the log to the clipboard. Nothing is sent anywhere.
   Paste it into your message.
2. Add:
   * what you did, step by step, starting from the start screen
   * what you expected, and what happened instead
   * a screenshot if you can (Win+Shift+S)
   * the project file (.kopercad or .novo), if it is not private. It is a small file.
3. After a crash, KoperCAD offers to open the crash report the next time you start it. You
   can also choose Help > **Open log folder** (%LOCALAPPDATA%\KoperCAD\logs) and send the
   newest crash-*.txt together with the project file.

Say how bad it was. We use these words:

* Lost work, crash, or cannot install or start: top priority.
* Cannot finish a normal task, or a wrong number is shown as if it were right:
  high priority.
* Everything else (looks odd, wording, slow, annoying): please still report it.

Send reports to the person who gave you the build. Do not worry about
duplicates.


5. Known limitations in this beta
---------------------------------

Please do not report these as new; do tell us if one of them blocks your work.

* Windows only. It has been tested mostly on automated builds; real-PC testing is
  what this beta is for.
* The dark theme is the supported theme. In the light theme some screens (Reports,
  the Cabinet toolbar) are not fully coloured for it.
* Numbers that are estimates are labelled as estimates. Please read the labels:
  - Heat and airflow come from a physical model, not a certified thermal
    calculation. The accuracy statement is in the program and in
    docs/thermal-validation.md. The 3D airflow result is marked "not validated".
  - Functional safety (PFHd, performance level) uses a closed-form estimate,
    "model, not exact". The ISO 13849-1 table values are not built in. This is not
    a replacement for SISTEMA.
  - Cable ampacity, derating and voltage-drop tables are planning values. Check
    them against the standard before you rely on them.
  - Wire lengths are placeholders (shown with ~) until the cabinet is laid out
    and the wires are routed ("Route all").
  - The standards checks (IEC 60204-1, NFPA 79, UL 508A) are a subset of rules,
    not a certification.
  - The motor start-up curve is an estimate.
* Weights and prices appear only when the part data has them.
* The parts library contains only what you import. Maker catalogues and EPLAN
  data are not included; see docs/PARTS-BULK-IMPORT.md to import your own.
* Report text is English. The assembly checklist and the reports can be switched
  to Dutch; the rest of the program is not translated.
* No EPLAN project import or export, no PLC program export, no symbol editor, no
  label printer drivers (label CSV and PDF exist), no several-people-at-once
  editing, no automatic updates (install the next build over this one).
* The 3D view has a slider for the section plane but no mouse-drag yet; heat
  arrows show no value when you hover them; the autosave interval is fixed; there
  is no custom symbol folder; terminal jumper numbers are automatic only.
* STEP import covers common shapes; some curved surfaces are approximated.
* Windows hands-on checks of the wheel timing and the live view are still being
  completed. Tell us if the Q wheel jumps to a wedge you only crossed.


6. For the curious
------------------

* Command line: kopercad-cli.exe in the portable zip (run it in a terminal; "kopercad-cli --help").
* Python: the program can be scripted; the module is called kopercad.
* Third-party software licences: THIRD_PARTY_NOTICES.txt next to this file.
* Source documents: docs\ in the repository, starting with docs/DESIGN.md.

KoperCAD is a work in progress by KoperVonk, released as open source under the MIT or
Apache-2.0 licence, at your option (LICENSE-MIT and LICENSE-APACHE).
