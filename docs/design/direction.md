# KoperCAD design direction

Design lead thread, 2026-10-16. Companion to the audit (`design/visual-audit-2026-10-16.md`). The token and widget spec stays `docs/design/design-system.md` (do not fork it); this page decides **what the product should feel like, what is visible by default, and how the work is cut into pull requests**. Where it amends the spec, section 10 says so and the spec is edited in the same PR.

## 1. One sentence

KoperCAD is a calm graphite workbench with one copper thread running through it: the drawing is the brightest thing, everything else steps back, and the program shows you three things at a time, not thirty.

Three tests every screen has to pass (reviewers use them):

1. **Squint test.** Blur the screenshot. You should still see: the drawing, then the one active thing (tool, selection, primary action) in copper, then quiet panels. If three areas compete, remove something.
2. **Count test.** No toolbar shows more than 7 controls at rest, no menu more than 12 items, no panel more than 3 text styles. The rest lives under "More", in a popover or in the command field.
3. **Overlap test.** Nothing floats over a toolbar, a title block or another panel. Floating things dock to a named slot (section 6).

## 2. Brand identity

**Name treatment.** `KoperCAD` set in Inter SemiBold: "Koper" in `text.primary`, "CAD" in `accent` (as in `assets/branding/lockup.svg`). Never all one colour in the product; never split across two lines. Second line, when there is room: `BY KOPERVONK` in the `label` style (10.5 pt caps, +1.2 spacing) in `text.secondary`.

**Mark.** Keep the "Nova" mark (four tapered rays on a copper junction dot: the dot that joins wires). It is already drawn in code (`brand::paint_mark`, no image loader needed). Rules: minimum 16 pt in the UI; the copper dot is the only copper in it; on the light theme the rays use `text.primary` and the dot uses the light accent; keep clear space of one ray width; no outlines, shadows or animation except the single start-up fade. Where it appears: top bar (20 pt, left of the first menu, it opens the About card), start screen (64 pt above the wordmark), About, splash/loading, window icon, installer, PDF export footer (optional, off by default, "Made with KoperCAD").

**Voice.** Short, concrete, no exclamation marks, no "Oops". Labels are verbs or nouns the electrician already uses ("Place all", "Wire list"), not product jargon.

**Copper means "here".** The copper budget per screen: the selection, the active tool or mode, the keyboard focus ring, and at most one primary button. Copper never marks status (that is the status colours), never decorates headings or numbers, never tints a whole toolbar chip row. Things that are only "switched on" (visibility layers, filters) use a neutral treatment: `surface.hover` fill and an `accent` check icon, no wash.

## 3. Palette

Roles and exact values are in design-system.md section 2.1 (dark and light). Summary of the intent, plus the three roles this direction adds:

| Role | Dark | Light | Rule |
| --- | --- | --- | --- |
| Surfaces | `#0E0E10` / `#151517` / `#1B1B1E` / `#232327` / `#2D2D32` | `#FCFBFA` / `#F2F1EE` / `#FFFFFF` / `#E9E7E3` / `#DFDCD7` | Step by one surface per level of containment. Drawing area is `sunken`. |
| Accent copper | `#C88A5E` | `#9A5427` | See "copper means here". |
| Status | success `#7FB086`, warning `#D6A55C`, error `#E0766C`, info `#8FA8C8` | `#276B36`, `#7F5408`, `#A82F25`, `#285488` | Always with an icon or words. |
| **Heat (new role set)** | magma-like ramp, cold `#2B0F54` to hot `#FCFDBF` | same ramp, drawn on a light card | Only for temperature. Never reused for warning or error. A hot spot over the limit adds a `warning` outline and a glyph, it does not recolour the ramp. Legend always shows units and the ambient value. Estimate mode (no solver run) says "Estimate" in the legend title and uses a hatched swatch. |
| **Net colours (schematic)** | per `CANVAS_DARK` potentials | per `CANVAS_LIGHT` | Wire blue is the drawing's, not the UI's. Cable cores are drawn in the cable's own neutral (`symbol` at 60 percent) with the label in `accent` only when the cable is selected. |
| **Drawing paper** | sheet visibly lighter than the surround (8 L* steps) | white on warm grey | The sheet is an object on a desk. |

The dark theme is the default and the one screenshots are judged on; the light theme must be complete (it exists in `novo-theme`) and is reached from Preferences (PR 3).

## 4. Type, icons, space

- **Type scale** is the spec's: display 20, title 15, body 13, caption 11, label 10.5 caps, mono 12. Decision: drop body to **12.5 in Compact** only; nothing else changes. Numbers in columns are mono and right aligned. At most three styles per panel.
- **Drawing text** has a floor: tags and values never render below **9 px on screen** at any zoom; at "fit" the sheet is scaled so the sheet width fills the canvas minus 24 pt (audit E1).
- **Icons**: Phosphor, regular at rest, fill when active; 16 pt in buttons, 14 in rows, 20 in panel headers; electrical concepts in `icons::custom` on the same grid. Icon-only buttons always have a tooltip with name and key. A toolbar button is **icon + word only for the primary action of that toolbar**, icon-only for the others (tooltips carry the words); this alone removes about a third of the width of the Cabinet bar.
- **Space**: the spec's 4 pt grid. Decision: a panel's inner padding is `M` (8), between groups `L` (12), a toolbar is **32 pt high** with 4 pt gaps, a panel header is 28 pt, a table row is 24 pt (Standard). Four control heights are allowed and nothing else: 20 (Compact), 22 (Standard), 28 (toolbar button with label), 36 (Touch).

## 5. Density modes

Compact 20 / Standard 22 / Touch 36 as in the spec. Defaults: Standard; first run on a window narrower than 1500 px or shorter than 800 px starts in **Compact** (a one-time suggestion toast: "Compact fits more on this screen. Change in Preferences > Appearance."). Commissioning on a touch device can switch to Touch for the Commissioning workspace only (it already has "Large buttons": replace by the density setting, same code path).

## 6. Layout: slots, panels and toolbars

The window has five named slots. Anything that appears on screen occupies one; nothing is positioned by hand.

```
+------------------------------------------------------------------+
| TOP BAR   mark  menus | workspace tabs | breadcrumb       search issues |
+--------+--------------------------------------------+------------+
| LEFT   | TOOLBAR (one row, 32 pt, contextual)       | RIGHT      |
| outline| +----------------------------------------+ | inspector  |
| pages  | | CANVAS (drawing / plate / 3D)          | | heat       |
| issues | |                          dock (bottom  | | properties |
|        | |                          centre)       | |            |
+--------+--------------------------------------------+------------+
| STATUS BAR: message left, zoom/coords/units right                |
+------------------------------------------------------------------+
```

Rules:

1. **Left slot**: Outline with tabs Pages, Issues, Changes, Commissioning. Pin by default at 1700 px and wider (as now); below that it is a **flyout opened from a panel icon button in the top bar** (not a floating button over the canvas: fixes G1 for good). The tab row never clips: tabs shrink to icons with tooltips below 280 pt width.
2. **Right slot**: one panel at a time, tabbed if there are several (Inspector, Heat, Properties). The Heat panel (#714) and the 3D tools use it. Width 280 default, resizable (panels thread's helper, #712), remembered per workspace.
3. **Toolbar**: one row, 32 pt, left to right: primary actions for the workspace (at most 4, first one labelled), a spacer, view toggles (segmented), then a "More" menu button. Everything else is under More or in the command field.
4. **Dock** (schematic only): bottom centre, tools as icon buttons with one segmented control for wiring mode (Corners | Lines, with icons). Messages never go in the dock; they go in the status bar, or as a toast for results.
5. **Panels** have a header (icon, title in `title`, trailing icon buttons for pin/close), then sections introduced by a `section_label`. No panel uses `Heading` or bare `label`.
6. **Floating overlays** (legends, view cube, badges) live in the four corners of the canvas, inset 12 pt, and never over a toolbar. Legends are cards on `raised` with `radius::SURFACE`.
7. **Dialogs**: 360, 480 or 640 wide only, one primary action, Esc closes.
8. **Tables**: the one `widgets::table` (to be built, PR 4): header on `panel` in caption caps, 24 pt rows, hairline rows, right-aligned mono numbers, sticky header, selected row `accent.wash`, column resize, ellipsis (never mid-word clipping).

## 7. Empty states

Every empty state has an icon (32 pt, `text.tertiary`), one sentence of what this is, one primary action, optionally one secondary link. Centred in the area, never top-aligned. Examples:

| Screen | Text | Primary |
| --- | --- | --- |
| No project | mark, "KoperCAD", `BY KOPERVONK` | New project; Open project; Open example; recent list; "Try the fast keys" as a text link under them |
| Cabinet, no plate | "A cabinet starts with a mounting plate." | "Set plate 600 x 800 mm" (with a size field) |
| 3D, no plate | the tab is disabled with the tooltip "Set the plate first (Cabinet)" | none |
| Reports, no data | "Nothing to list yet. Place devices and this fills itself." | "Go to Schematic" |
| Issues, none | "No issues." with the success check | none |

## 8. Progressive disclosure: what is visible by default

| Area | Visible at rest | Under More / elsewhere |
| --- | --- | --- |
| **Top bar** | mark, 7 menus, workspace tabs, breadcrumb, search field, issue pill | window actions |
| **Menus** | File, Edit, Draw, View (zoom, focus, full screen, marks, potentials, panels), **Simulate (new: motor start, heat, airflow, PLC program, fault scenarios, live potentials)**, Check, Reports, Help. Maximum 12 items per menu, section labels between groups, no permanently disabled items (hide or grey with a reason only when a project is open). Lessons commands leave the View menu and live under Help > Learn. | the rest through the command field |
| **Schematic dock** | Select, Place, Wire, wiring mode (Corners / Lines as icons), Text | all other tools: command field or Draw menu |
| **Cabinet toolbar** | Panel selector, **Place all** (primary, labelled), Plate / Table (segmented), Heat (toggle), **More** | More: Rules, Plate setup, Size enclosure, Route all, Rails and ducts, Cut list, Export. Heat controls (Map, Contours, Airflow, Badges, Simulate) move into the Heat panel in the right slot |
| **3D** | toolbar: Layers (popover), Move (mode toggle), Heat (toggle) | Layers popover holds Enclosure, Door, Wires, Shadows as checkboxes; "?" badges only when hovering a device or in a "Missing data" layer |
| **Inspector** | title, description, relations, part | Properties section hidden when empty; advanced fields behind "All properties" |
| **Reports** | report list, table, filter, Excel/CSV as one "Export" button | settings |
| **Commissioning** | progress bar, Next open, list grouped by page | Report PDF/CSV and "By" under a header menu |
| **Heat panel** | scale, hottest device, total loss (hidden when no data), Simulate | contour, airflow, badge toggles, explanation text under an info icon |

## 9. Plan of small PRs

Each PR names where the user sees it, carries before/after screenshots at 1366x768 and 1920x1080, and states what was not verified on Windows. Sizes: S under 200 lines, M under 600.

| # | PR | Size | Owner | Where the user sees it |
| --- | --- | --- | --- | --- |
| 1 | Docs: audit, direction, design-system amendments (section 10), screenshot script resolution option | S | design lead | `docs/design/`, `scripts/screenshot.sh` |
| 2 | **Pages flyout toggle moves to the top bar** (fixes G1) and the outline tab row never clips (G2) | S | design lead, with panels thread (#712 touches the same code: coordinate first) | top left, every workspace, window under 1700 px |
| 3 | **Appearance**: theme (Dark, Light, follow system) and density in Preferences, canvases follow the theme, first-run compact suggestion | M | design lead | Preferences > Appearance |
| 4 | **Start screen and brand**: mark in the top bar, start screen with mark and wordmark, About card, recent list | M | design lead | launch with no project; click the mark |
| 5 | `widgets::table` (sticky header, alignment, hairlines, ellipsis) + Reports migrated | M | design lead for the widget; Reports owner (R5 lane) for migration | Reports workspace |
| 6 | Commissioning column on shared widgets (progress bar, grouped list, one header menu) | S | Commissioning owner | Outline > Commissioning |
| 7 | Cabinet toolbar: primary row plus More, Heat controls into the Heat panel slot | M | cabinet owner with #714 | Cabinet workspace |
| 8 | View menu split into View and Simulate, no permanently disabled items | S | workspaces/workflow threads | menu bar |
| 9 | 3D Layers popover, Move as a mode, "?" badges to hover | S | 3D view owner | 3D workspace |
| 10 | Copper discipline pass: layer toggles neutral, cable cores neutral, duct anchors neutral, selected colours only | M | design lead with canvas thread | all canvases |
| 11 | Schematic legibility: fit-to-width, text floor, title block contrast, dock icons for wiring modes, messages to status bar | M | canvas and dock owners | Schematic |
| 12 | Light theme sweep: every screen in light at both sizes, fixes | M | design lead | Preferences > Light |

Dependencies: 2 before 7, 9 (they share the slots); 3 before 12; 5 before 6. 4 and 8 are independent. R3 reviews 3, 4, 5, 10 and 12 (theme and UI); design reviewer B reviews every PR that changes layout.

## 10. Amendments to `design-system.md`

1. Section 3 (States): a **visibility or filter toggle** is neutral (`surface.hover` fill, accent check icon); the `accent.wash` toggle is for tools and modes only.
2. Section 4 (Components): add `table` as a built widget (not only a description), `panel_header`, `toolbar` (32 pt) and `popover_menu` (More).
3. Section 2.3: toolbar 32 pt, panel header 28 pt, table row 24 pt.
4. Section 2.1: add the Heat and Drawing paper rows from section 3 above.
5. Add a section "Slots" with the layout in section 6.

## 11. How this is checked

- `scripts/screenshot.sh` captures any screen at a chosen resolution (`SCREENSHOT_RES=1366x768`), used for before/after in every PR. A scripted pass over all workspaces at 1366x768 and 1920x1080 in dark and light is part of PR 12 and is re-run after every layout PR.
- The three tests in section 1 are the review checklist.
- Honest limits: captures come from a Linux software renderer. Windows font hinting, 125 and 150 percent scaling and real GPU colour are not covered; Tadjiro's own Windows run of the experimental build is the check for those, so every PR says what to look at.
