# PR 1870 evidence (source at git e2e67de468)

Binary built from head e2e67de468e24a135e2f03ffbfff3734f5b8ca30; scenario `qa/scenarios/1870-step-health-line.qa` at 1366x768 and 1920x1080, 0 failed. The 6 findings are S4 "may overlap" notes: the start screen's New project button sits behind the Parts window, on purpose (scenario 700 shows the same). Pictures (not committed): `/mnt/project-files/qa/pr1870-e2e67de4/shots/` (clean-plate, details, gaps, broken).

## Where the user finds it
- Parts window, Add CAD files: each STEP file gets a colour swatch, a verdict (Reads cleanly / Reads with gaps / Cannot be read) and its product name; details behind the "?" (the "details" picture).
- Command line, a whole folder (STEP files and ZIPs): `kopercad-cli parts step-health [folder] [--report health.md]`; default folder `Documents\KoperCAD-tests\step-samples`.

## Tested
fmt, clippy -D warnings (interop, cli, app), novo-interop health tests (13), CLI parts tests, app parts tests (78), this QA scenario with hand-made fixtures. Run on three real Rittal ZIPs (private, outside the repo): TS 8 enclosure and a rail read, with 12 of 1038 enclosure faces drawn flat (2 degenerate tori) and 1 face unreadable; mounting plate and rail read cleanly.

## Review points since the first head
One file read per STEP file (the size comes from the health report), the report is keyed by file size and modification time, the header reader handles comments, quotes and brackets, and the CLI help matches the file names it accepts.

## Not tested
- The "unsupported" list is a name heuristic.
- Colours are read but not yet applied to the 3D drawing.
- The Parts window line is for STEP files added directly, not for STEP files inside a ZIP (the CLI reads those).
- Full workspace test run (cloud disk); CI covers it.
