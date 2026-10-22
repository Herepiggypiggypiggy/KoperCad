# Progress

- 2026-10-22: PR 1 pushed (see HANDOFF.md). Linux QA at 1366x768 and 1920x1080 (scenario 74) passes.

## Only real Windows can prove
1. Installer: no UAC prompt; per-user install; Start menu has KoperCAD and "KoperCAD (safe graphics)"; uninstall asks about settings and leaves projects.
2. Upgrade over a running copy closes it (CloseApplications) and leaves one shortcut.
3. Portable zip: `kopercad.portable` makes settings and logs land in `data\` next to the exe, nothing in %LOCALAPPDATA%.
4. `--safe-graphics` picks the Basic Render Driver (WARP) on a real Windows PC and the app is usable.
5. Double-click a .kopercad / .novo file opens it after choosing the association task.
6. HiDPI 150% and 200% laptop screens; no console flash on start (windows subsystem).
