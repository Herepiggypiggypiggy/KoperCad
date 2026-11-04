# PR 1870 evidence (source at git a7f17cbc8a)

Binary built from head a7f17cbc8a22ed030119bd1a4600ea9aa8be977a; scenario `qa/scenarios/1870-step-health-line.qa` at 1366x768 and 1920x1080, 0 failed. The 6 findings are S4 "may overlap" notes: the start screen's New project button sits behind the Parts window, which is on purpose (scenario 700 shows the same). Pictures (not committed): `/mnt/project-files/qa/pr1870-a7f17cbc/shots/` (clean-plate, details, gaps, broken).

## Where the user finds it
- Parts window, Add CAD files: each STEP file gets a colour swatch, a verdict (Reads cleanly / Reads with gaps / Cannot be read) and its product name; details behind the "?" (shown in the "details" picture).
- Command line, whole folder: `kopercad-cli parts step-health [folder] [--report health.md]`; default folder `Documents\KoperCAD-tests\step-samples`.

## Tested
fmt, clippy -D warnings (interop, cli, app), novo-interop lib tests (170), CLI parts tests, app parts_cad tests, this QA scenario with hand-made fixtures (clean coloured plate, file with unreadable surface kinds, broken file).

## Not tested
- Real Rittal files (not yet received); the "unsupported" list is a name heuristic.
- STEP files inside a ZIP (no report).
- Colours are read but not yet applied to the 3D drawing.
- Full workspace test run (cloud disk); CI covers it.
