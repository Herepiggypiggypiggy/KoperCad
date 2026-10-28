# Unreleased

Nothing yet after v0.1.0-rc6. Add a line here for each change a tester will notice, then move the
lines into `v0.1.0-<next>.md` when the next release candidate is cut.

(Earlier contents of this file, wire-rule, I/O import and cable-length items, shipped in the
rc3 builds and are in `v0.1.0-rc3.md` or the history of this file.)

## Installer closes a running KoperCAD (not tested on real Windows)

Installing an update or uninstalling now closes a running KoperCAD of the install folder first
(`%LOCALAPPDATA%\Programs\KoperCAD`): a normal close, and after ten seconds the process is ended.
**Save your project first**; unsaved work is not asked about. A portable copy in another folder is
not touched. The script was built by CI but has not run on real Windows hardware: if the installer
still reports a locked `kopercad.exe`, close KoperCAD by hand and report it with Help > Copy problem report.
