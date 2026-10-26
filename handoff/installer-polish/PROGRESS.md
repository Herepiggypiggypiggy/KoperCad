# Progress

- 2026-10-22: PR 1 pushed (see HANDOFF.md). Linux QA at 1366x768 and 1920x1080 (scenario 76) passes.

## Only real Windows can prove
1. Installer: no UAC prompt; per-user install; Start menu has KoperCAD and "KoperCAD (safe graphics)"; uninstall asks about settings and leaves projects.
2. Upgrade over a running copy closes it (CloseApplications) and leaves one shortcut.
3. Portable zip: `kopercad.portable` makes settings and logs land in `data\` next to the exe, nothing in %LOCALAPPDATA%.
4. `--safe-graphics` picks the Basic Render Driver (WARP) on a real Windows PC and the app is usable.
5. Double-click a .kopercad / .novo file opens it after choosing the association task.
6. HiDPI 150% and 200% laptop screens; no console flash on start (windows subsystem).
6b. Updating over a running copy with unsaved work: CloseApplications asks the app to close; confirm the unsaved-project prompt or recovery offer still appears.
- 2026-10-23: PR 2 (update check, installer script tests, scenarios 210/211). 12 unit tests pass, clippy clean, QA 4 runs 0 failed. Windows-only list above still stands, plus: Check for updates against real GitHub from a Windows PC behind a proxy.

- 2026-10-26: clean-profile pass, PR #1515 (installer CI extended, two uninstall fixes). Installer job green on 9c01d91.
