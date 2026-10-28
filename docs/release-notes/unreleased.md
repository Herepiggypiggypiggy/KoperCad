# Unreleased

Nothing yet after v0.1.0-rc6. Add a line here for each change a tester will notice, then move the
lines into `v0.1.0-<next>.md` when the next release candidate is cut.

(Earlier contents of this file, wire-rule, I/O import and cable-length items, shipped in the
rc3 builds and are in `v0.1.0-rc3.md` or the history of this file.)

## Installer closes a running KoperCAD (not tested on real Windows)

Installing an update or uninstalling now closes a running KoperCAD of the install folder
(`%LOCALAPPDATA%\Programs\KoperCAD`) first. If you have unsaved changes KoperCAD asks first: answer
within a minute (ten seconds in a silent install) or the installer ends it, and your recovery copy is
offered at the next start. **Save your project first.** A portable copy in another folder is not touched.
If company policy blocks PowerShell, the installer falls back to the old behaviour (a locked
`kopercad.exe`: close KoperCAD by hand). The script was built by CI but has not run on real Windows
hardware: report problems with Help > Copy problem report.
