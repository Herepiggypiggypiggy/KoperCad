# Dock everything on the right (requests 1, 3, 7 of 2026-11-02)

Scope: every popup window can be docked (Schematic, Cabinet, 3D), folds up, and opens docked at the right edge by default. Full inventory: `docs/` copy in `handoff/dock-everything/INVENTORY.md` (project mount: `/mnt/project-files/dock/popup-inventory.md`).

Done (PR on branch claude/project-thread-bb5jpo): `panel::Dialog` wrapper (53 command windows), forms/settings modals dockable, fold arrow on every pane, default right dock, 5 more cards dockable.

Next:
- Per-window QA scenarios at 1366x768 and 1920x1080 (none yet for docked dialogs).
- Wide windows (print preview, designer ...) are dockable but float by default; consider a bottom default.
- Requests 2 (help popups) and 8 (3D Layers into the rail, #1808) landed on main in their own PRs.
- Dock strip default width 320 is narrow for 420+ dialogs; consider remembering a per-dialog width.
