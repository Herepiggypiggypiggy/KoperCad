# Workspaces, menus and navigation: roadmap

Scope: the information architecture of the window. Workspaces (Schematic, Cabinet, 3D, Reports, Commissioning)
with a switcher and a remembered layout each, a menu bar generated from the one command registry, a status bar,
a consistent panel system, and a command surface that lists everything. Design: `docs/design/workspaces.md`.

Plan (design §10), one small draft PR each, no build:
1. Design doc + DESIGN.md line edits (this PR)
2. Workspace enum, switcher, remembered layouts (`app/workspace.rs`, `settings.rs`)
3. One registry: `menu` on `CommandInfo`, `Action` folded into `COMMANDS`, invariant tests
4. Menu bar generated from the registry
5. Status bar (HUD and saved text move into it)
6. Panel registry, drawer tabs per workspace, Alt+digit, F6, Reset layout
7. Command surface: menu path column, workspace and report words, Dutch aliases
8. Reports workspace  9. 3D workspace  10. Commissioning workspace  11. Keys (Ctrl+1..5)

Open: Workflow must agree to move Ctrl+3/4/5 off the drawer tabs (step 11).
