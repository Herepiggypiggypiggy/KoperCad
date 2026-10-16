# KoperCAD visual audit, 2026-10-16

Candid critique of the app on main (7e0ff43, after #715). Written by the design lead thread.

**How it was captured.** The real debug build, rendered in Xvfb with a software Vulkan driver (llvmpipe), window maximised at 1366x768 and 1920x1080, driven by clicks and keys (example project, Cabinet plate set and "Place all", 3D, heat, Learn, command field, menus). Pictures are in `docs/design/audit-2026-10-16/` (`1366x768-*.png`, `1920x1080-*.png`), referred to below as `[1366 reports]` and so on.

**What this does and does not prove.** It proves layout, overlap, clipping, hierarchy and colour use. It does not prove Windows font rendering, DPI scaling (125 percent, 150 percent are the common Windows settings and were not captured), or GPU colour. Nothing here was run on Windows. The default window is 1280x800, so on a 768 pixel high laptop screen the bottom of the window is off screen until the user maximises it.

## Verdict

The design system (`docs/design/design-system.md`, `novo-theme`) is good and mostly not applied. The theme crate has roles, a light theme, density, 20 widgets and a gallery. The app uses egui defaults plus the dark palette in most panels: no light theme or density setting exists in Preferences, the logo mark is never drawn anywhere in the app, and tables, drawers and the Cabinet and Commissioning panels are laid out ad hoc. The result reads as a pile of features. The Learn dialog and the command field are the two screens that look like they belong to the same product; they are the standard to bring the rest up to.

Five root causes explain most of the mess:

1. **Chrome competes with the drawing and with itself.** Copper is used for selection, active tool, toggled chips, the primary button, headings and "problem" tints at once, so it no longer means "here".
2. **Everything is visible at once.** The Cabinet toolbar shows 12 controls in 6 groups, 18 with Heat on; the View menu has 33 entries; the 3D view shows 5 checked chips.
3. **No shared widgets for tables, tab rows and toolbars.** Each panel invents its own sizes (20, 22, 28 and 36 pt controls appear on one screen).
4. **Overlays are placed without knowing what is under them** (the "Pages" button over the Cabinet toolbar).
5. **No brand.** Start screen, About and header have no mark, no wordmark, no copper moment.

## Problems, by severity

S1 = looks broken, S2 = looks messy or costs time, S3 = polish. "Owner" is the thread that should clean it up.

### Global

| # | Sev | Problem | Where | Owner |
| --- | --- | --- | --- | --- |
| G1 | S1 | The floating "Pages" toggle is drawn on top of the Cabinet toolbar ("PANEL" label and "Pages" overlap, unreadable) whenever the window is under 1700 px wide, so on every laptop. | `[1366 cab]`, `[1366 3dheat]`; `outline_flyout` in `app.rs` | Panels thread + design lead (PR 2 below) |
| G2 | S1 | Drawer tab row is clipped: "Commissioning" shows as "Com", "Pin" collides with the last tab. Page names are cut mid-word ("Terminal diagram · =A1+CA") with no ellipsis. | `[1366 commissioning]`, `[1920 cabheat]` | Panels thread |
| G3 | S2 | The View menu has 33 entries in one flat list (view toggles, simulations, heat, lessons that are greyed out, commands that do things); it runs off the bottom of a 768 px screen. Disabled lesson commands are listed permanently. | `[1366 menuview]` | Workflow + Layout |
| G4 | S2 | Copper is overused: selected top-bar tab is a grey chip, but toggled chips, the Accept button, headings in drawers, progress rows, the Pages row text, cable cores on the drawing and the orange duct blocks in Cabinet are all copper. Selection colour should not appear on things that are not selected. | everywhere | Design lead (direction doc) |
| G5 | S2 | Control sizes do not agree: toolbar buttons 22 pt, "Accept" 28 pt, Commissioning buttons 36 to 44 pt, "Next open" larger again. | `[1366 commissioning]`, `[1366 cab]` | Each panel owner, with the shared widgets |
| G6 | S2 | No Preferences entry for theme or density; the light theme and Compact/Touch exist in `novo-theme` but cannot be reached. An engine without a UI entry is not done. | `preferences.rs` | Design lead (PR 3) |
| G7 | S2 | The search box is a small "Search" button at 1366 and a wide field with "Ctrl+K" at 1920; the window title bar and the breadcrumb both repeat the project name; the breadcrumb truncates to "Motor starter…" with plenty of free space beside it. | top bar | Workspaces thread |
| G8 | S3 | The disabled "3D" tab is almost the same grey as the inactive "Cabinet" tab, so it looks clickable-but-dim, not unavailable. Needs the reason in the tooltip and a clearly lower contrast, or a hint "Set the plate first". | top bar | Workspaces thread |
| G9 | S3 | Default window 1280x800 does not fit 1366x768 screens. | `lib.rs` | App shell |

### Start screen and brand

| # | Sev | Problem |
| --- | --- | --- |
| B1 | S2 | `[1366 start]`: a left-aligned grey stack of four full-width buttons in the upper middle of a blank page. No mark, no wordmark, no "by KoperVonk", no copper. This is the first thing a person sees. The "Try the fast keys" button is styled as an equal of "New project". |
| B2 | S2 | The mark exists (`brand::paint_mark`, `assets/branding/lockup.svg`) and is used nowhere in the UI. The top bar starts with a text menu "KoperCAD" that looks like any other menu. |
| B3 | S3 | The native Windows title bar repeats the name and project; nothing wrong, but the app also draws its own bar below it, so 60 px are spent before the drawing starts. |

### Schematic editor

| # | Sev | Problem |
| --- | --- | --- |
| E1 | S2 | `[1366 schem]`: at "fit" the sheet fills about 60 percent of the canvas and symbol text is about 7 px: the drawing is the least legible thing on screen, which inverts the first design rule. The sheet should fit the canvas (width or height, whichever binds) and tag text should have a minimum screen size. |
| E2 | S2 | The title block text and the sheet's column numbers are very low contrast (text.tertiary on near-black). Fine for chrome, too faint for content the user will check. |
| E3 | S2 | The floating dock ("Corners | Lines | T") sits over the sheet frame and title block area. "Corners" and "Lines" are words with no icon and no indication that they are two modes of one thing. When a message is shown (`=A1+CAB-K1 elsewhere: ...`) it extends the dock sideways instead of using the status area, so the controls move. |
| E4 | S3 | Inspector `[1366 inspector]` is the best panel in the app (hierarchy, relation cards, part). Problems: "Pin" is a bare word at the top right; section labels are fine; "PROPERTIES: None" is shown as an empty section instead of being hidden; the panel floats over the sheet's right margin and covers column 10. |
| E5 | S3 | Cable cores drawn in thick copper-orange bars dominate the schematic and read as "selected" (see G4). |

### Cabinet

| # | Sev | Problem |
| --- | --- | --- |
| C1 | S1 | G1 overlap. |
| C2 | S2 | `[1366 3dheat]`: toolbar of 12 controls in 5 captioned groups (PANEL, PLACE, ARRANGE, ROUTE, VIEW, OUTPUT) plus a second row of 6 Heat controls. "Place all" greyed out, "Rules" and "Plate setup" and "Size enclosure" are three kinds of setup. At 1366 it wraps into two rows plus the status row, using about 140 px. Needs a primary row (the 3 to 4 things you do all day) and the rest under "More". |
| C3 | S2 | The "Accept Enter / Cancel Esc" preview bar uses the biggest, most saturated control in the app for what is a routine confirmation, and the row of pills below it ("0 placed", "7 unplaced", "0 problems") uses three different pill styles. `[1366 cabinetfull]` |
| C4 | S2 | Rails and ducts are drawn as pairs of overlapping outlines; the copper squares at their left ends have no meaning on screen (they are rail anchors) and look like selection. Devices are dark blue boxes with no tag until selected, so a placed plate is unreadable. |
| C5 | S2 | Unplaced list: rows read "F1 / 45 x 90 mm (guessed)" in two lines with the same weight; "guessed" repeated seven times is noise: one banner would do. |
| C6 | S2 | Heat map `[1366 3dheat]`: legend card "Total loss -0 W" (negative zero is a bug, and the figure is meaningless when 7 devices have no loss data), badges overlap the plate edge and each other and lose their device names ("46 °C" with no tag in `[1366 menucheck]`), the contour label "40" floats alone. The explanatory sentence is 4 lines in a 140 px column. |
| C7 | S3 | The empty state "Set the size of the mounting plate to start placing devices." is a grey sentence at the top; the action sits at top left, far away. |

### 3D

| # | Sev | Problem |
| --- | --- | --- |
| D1 | S2 | `[1366 c-3d]`: five checked chips (Enclosure, Door, Wires, Move, Shadows) all copper-washed, so nothing is "on" by contrast; "Move" is a mode, not a layer, and should not look like a layer toggle. |
| D2 | S2 | The "?" badges on every device (no part data) are louder than the devices; the same fact is stated once as a banner in Cabinet. |
| D3 | S2 | The view cube is tiny (about 80 px) with unreadable face names at 1366 (#710 addresses letters). The room is a flat near-black with no ground reference except the faint grid; the devices are a flat light grey: the scene has little depth for a "3D" view. (3D look thread owns this.) |
| D4 | S3 | There is no panel or inspector in 3D: the Heat panel (#714) and Move tools will have to land somewhere. Needs a defined right-hand slot (see direction doc, panel rules). |

### Reports

| # | Sev | Problem |
| --- | --- | --- |
| R1 | S1 | `[1366 reports]`: tables are raw labels in a grid with no header style, no row hairlines, no alignment: "Page / Title / Page group / Panel / Kind" headers are centred over left-aligned data, so "Generated: toc" sits under "Page group". The Excel and CSV buttons touch the window edge at 1366. A second table ("PANELS") is bare text. |
| R2 | S2 | The left list (report names with counts) is good: keep as the pattern. Selected state is the copper wash. |
| R3 | S2 | Row text is 13 pt with 22 pt spacing but there is no zebra or hairline, so 50 rows will be unreadable; numbers are not right aligned or monospaced. |

### Commissioning drawer

| # | Sev | Problem |
| --- | --- | --- |
| M1 | S1 | G2 clipping. |
| M2 | S2 | `[1920 commissioning]`: progress is two dark pills with copper text and a faded circle, no bar; "Large buttons" checkbox, "By" field, "Punch list" heading, "Report: PDF CSV" and "Next open" are five different styles in a 250 px column. |
| M3 | S2 | The list of 76 checks is 36 px per row, each a hollow circle and a name like "Wire#34": no grouping by page or type, no filter, no state colour. |

### Command field and Learn (the good ones)

The command field `[1366 palette]` and Learn dialog `[1366 c-learn]` have clear hierarchy, consistent row heights, readable key hints and sensible grouping. Small issues: disabled command "Rules: Comes later" should not be listed; the Learn dialog is about 660 px wide, not one of the three dialog widths in the design system; the dialog lists a "Show draft lessons" checkbox to everyone.

## What to keep

- The dark graphite surfaces and the single copper accent: right choice, wrong amount.
- The Inspector's relation cards and the Reports list pattern.
- Command field, Learn dialog, issue pill ("No issues"), status text style.
- Keycap hints (`kbd`), Phosphor icon set already bundled.

## Priorities

1. G1, G2: broken layouts (small, safe fixes).
2. R1, M2: tables and the commissioning column use shared widgets.
3. Start screen, header and About with the brand (B1, B2), Preferences theme and density (G6).
4. Progressive disclosure of the Cabinet toolbar, View menu and 3D chips (C2, G3, D1).
5. Copper discipline (G4, E5, C4) and drawing legibility (E1, E2).

The plan, with PR sizes and owners, is in `docs/design/direction.md` section 9.
