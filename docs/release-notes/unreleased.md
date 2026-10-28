# Unreleased

Nothing yet after v0.1.0-rc6. Add a line here for each change a tester will notice, then move the
lines into `v0.1.0-<next>.md` when the next release candidate is cut.

(Earlier contents of this file, wire-rule, I/O import and cable-length items, shipped in the
rc3 builds and are in `v0.1.0-rc3.md` or the history of this file.)

## Start-up and experimental builds (PR #1580)

- **Windows: DirectX 12 is tried before Vulkan** for the same graphics adapter. This changes the graphics path of every Windows user and is **not tested on real Windows hardware yet**. It follows a hardware report: on an Intel UHD laptop (driver 31.0.101.2125) the Vulkan path crashed the whole program with no window and no message, while DirectX 12 and safe graphics started fine. Help > About and Copy problem report show the adapter and backend in use.
- **A start that never draws its window is noticed.** KoperCAD leaves a marker while it starts; if the last start died (or was ended in Task Manager), the next start asks "Start with safe graphics now?" and tries the adapter that was in use last from then on. An adapter that later reaches the first frames is trusted again. Help > About lists adapters tried last. A crash inside the graphics driver itself still cannot be caught; the log shows which adapter step was running.
- Experimental zip: now with `run.cmd`, `run-safe-graphics.cmd` and readmes; the duplicate `novoelectric.exe` and the 74 MB canvas demo are gone (the demo is a separate file on the release page). `run-latest.ps1`, `setup-new-pc.ps1` and `build-local.ps1` need no GitHub sign-in any more.
