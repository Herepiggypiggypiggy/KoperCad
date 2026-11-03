# Redesign boards against main (2026-11-03)

Boards: the design canvas of PR #1566 (`docs/design/visual-redesign/images/*.png`, sources in `mockups/`). Main: 29ddcf5, screenshots from the QA harness at 1366x768 (`/mnt/project-files/qa/start-screen/audit-*`).

| Board | Designed | On main | Gap, and what closes it |
| --- | --- | --- | --- |
| Start screen | Tiles (New, Open, Open example, Try the fast keys), recent projects as cards with a picture of the first page, four capability cards, hints | **Was a text list** (320 pt column of buttons and `weak` lines) | Built in this lane: `app/start_screen.rs`. Cards read the project in the background and draw symbols and wires of the first schematic page; no sheet frame or text in the picture. Capability cards open their lesson. |
| App menu (two panes) | Areas on the left, commands on the right, search, About and Preferences in the footer | Built (#1680): two panes, search, footer | The File pane is still a plain list. The board shows tiles for New / Open / Open example, a Recent list, Import / Export / Print tiles and a "Move data in and out" group. **Not built** (next PR, app menu File pane). |
| Top bar at every width | Mark with caret, Undo and Redo, workspace segment (name only on the active one), breadcrumb pill with page switcher, search pill, issues pill, what is new, help | Built (#1684, #1815) but different on purpose: Undo and Redo and the workspaces are icon-over-caption tiles (owner request, 58 pt), the eight menu words are still shown | The menu words are on by default (`MENU_NAMES_DEFAULT = true`); the doc flips it in lane V7. **Open decision for Tadjiro**: keep captions and flip the default to hidden, or keep the words. |
| Rails per workspace | Per-workspace rails with flyouts and badges | Built (#1683, #1688, #1690, V3a to V3d) | Add-on slot and keyboard reach checks are listed in V7. |
| Place palette | Categories, real IEC symbols, preview pane, recents and macros first | Built (#1681) | Not re-shot in this audit; QA 07 and 12 cover it. |
| Command palette | Icons, type chip, menu path chip, key cap, pinned tiles when empty | Built: icons, chips and key caps seen in `04-command-search`; pinned tiles when the field is empty not checked | Check pinned tiles in V7. |
| Export centre | Format tiles, options, preview, one primary action | Built (#1752) | Matches in layout; the preview pane shows a hint until a format is picked. |
| Import centre | Same shape | Built (#1758) | |
| Windows: drag, dock, resize | Title-bar drag, dock left and right, resize | Built (V1 and follow-ups) | |
| Menu census | 152 rows | 186 rows (V4 PRs) | View (65) and Simulate (25) remain. |
| 3D workspace and side panel | Rail, side panel with foldable sections, status chip | Rail built; side panel in the 3D lane | Owned by the 3D threads. |
| Icon set | 40 drawn icons | Built (V0), gallery under Ctrl+K "Icon gallery" | |
| Add-ons manager and in-app | Manager window, add-ons slot | Built (add-on manager) | |

Honest limits: the cards on the start screen draw the first schematic page from the project file as symbols and wires only. The board shows the sheet with frame and title block, which needs the page renderer of the print path; a later step.
