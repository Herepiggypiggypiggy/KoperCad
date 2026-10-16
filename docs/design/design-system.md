# KoperCAD design system

The visual language of the whole app, in one place: what every panel, dialog
and canvas draws with, and the rules that keep them looking like one program.
The code is the `novo-theme` crate; this page is the spec it is built from and
audited against. It supersedes the visual parts of
[`ui-guidelines.md`](ui-guidelines.md) (that page keeps the layout, branding
and label-placement notes, and points here for tokens). Where the app lives on
screen and how work flows is `docs/DESIGN.md` and `docs/WORKFLOW.md`; this page
is only about how it looks and feels.

Owner: the design system thread. Coordinates with layout and branding (marks,
logo), the 3D look thread (materials follow the same colour roles) and the
workspaces thread (workspace chrome uses these components).

## 1. Character

KoperCAD should feel like a precise instrument: **calm, dense, exact**.

1. **The drawing is the brightest, sharpest thing on screen.** Chrome is quiet
   graphite or warm paper; it never competes with the schematic.
2. **Hierarchy from value, space and weight, not colour.** One surface step
   separates a region, one hairline separates a row, one weight step separates
   a title from its body. If a screen needs a second accent colour to make
   sense, its layout is wrong.
3. **One accent, copper, rare.** Copper means "here": focus, the active tool,
   the selection, the one primary action. It is never decoration and never a
   status.
4. **Status colours mean status**, and always travel with an icon or words.
5. **Everything lines up.** One 4 pt spacing grid, one type scale, one radius
   per kind of surface. Two things that look alike behave alike and are built
   from the same widget.
6. **Dense, not cramped.** An engineer scans hundreds of rows a day. Rows are
   short and aligned, but each has room to breathe horizontally and every hit
   target is at least 20 pt tall.
7. **Quiet motion.** 80 to 120 ms ease for hover, focus and panel changes.
   Nothing bounces, nothing loops except live-simulation state and the lesson
   pointer's pulse (section 10); the lesson card's check flash runs once.
   Reduce motion turns all of it off, and the pointer then holds still.

## 2. Tokens

Screens never write a raw colour, size or radius. They use the names below
(`novo_theme::tokens`, `novo_theme::Palette`, `novo_theme::space`). A raw
value in UI code is a review comment, and a test fails the build on one in
`novo-app`.

### 2.1 Colour roles

Roles, not numbered greys. A role keeps its meaning in both themes; its value
changes. (Existing `BG_0`..`BG_4` map one to one onto the first five surface
roles, so nothing breaks while screens migrate.)

| Role | Dark | Light | Used for |
| --- | --- | --- | --- |
| `surface.sunken` (was `BG_0`) | `#0E0E10` | `#FCFBFA` | Text fields, lists, tables, the page behind the sheet |
| `surface.panel` (`BG_1`) | `#151517` | `#F2F1EE` | Top bar, outline, drawer |
| `surface.raised` (`BG_2`) | `#1B1B1E` | `#FFFFFF` | Floating: inspector, dock, menus, tooltips, dialogs |
| `surface.control` (`BG_3`) | `#232327` | `#E9E7E3` | Buttons and fields at rest |
| `surface.hover` (`BG_4`) | `#2D2D32` | `#DFDCD7` | Hovered controls, pressed rows |
| `border.subtle` (`BORDER`) | `#25252A` | `#DAD8D3` | Hairlines between regions |
| `border.strong` (`BORDER_STRONG`) | `#3B3B41` | `#B5B1AA` | Hovered and focused outlines, inputs |
| `text.primary` | `#E3E1DC` | `#1E1D1B` | Body text, values |
| `text.secondary` | `#989691` | `#5A5853` | Labels, hints, units, section labels |
| `text.tertiary` | `#858380` | `#66645F` | Placeholders, coordinates, disabled |
| `accent` | `#C88A5E` | `#9A5427` | Focus, active tool, selection, primary action, logo |
| `accent.hover` | `#DBA27C` | `#82441D` | Accent text and hairlines under the pointer |
| `accent.wash` | `#3A2E26` | `#F4E4D7` | Selected rows, selected text |
| `on.accent` | `#14100D` | `#FFFFFF` | Text on an accent fill |
| `info` | `#8FA8C8` | `#285488` | Links, cross references, hover on the canvas |
| `success` | `#7FB086` | `#276B36` | OK |
| `warning` | `#D6A55C` | `#7F5408` | Warning |
| `error` | `#E0766C` | `#A82F25` | Error |

Each status role also has a **wash** (the colour at 12% on the surface) for
inline banners and table row tints, so a banner never needs a new colour.

Rules the unit tests hold:

- `text.primary` at least 7:1 on every surface, `text.secondary` and
  `text.tertiary` at least 4.5:1, accent and status colours at least 4.5:1 on
  `sunken`, `panel` and `raised`, in both themes.
- `accent` on `accent.wash` (selected rows and toggles carry an accent icon and
  accent hover text) is at least 4.5:1 in both themes: 4.54:1 dark, 4.61:1
  light.
- Controls step away from the panel in both themes: hover is further from the panel's lightness than a resting control, which is further than the panel.
- No mark colour on the canvas equals the selection colour.
- The 3D view's room follows the theme through `Palette::room` (`Room3d`):
  background gradient (the sunken pair in dark, `#ECEAE6` to `#DEDCD7` in light),
  ground grid, label chip (fill at 85%, outline, text at 7:1), selection = accent,
  hover = info, edge darkening 45% dark and 55% light. Physical materials stay
  in `novo-view3d`; problems use the status colours with a glyph (docs/design/3d-look.md).

### 2.2 Type

Inter for interface text (bundled, so it looks the same everywhere), the
bundled monospace for device tags and part numbers, Phosphor for icons.

| Style | Size | Weight | Used for |
| --- | --- | --- | --- |
| `display` | 20 pt | SemiBold | Empty states, About, first-run |
| `title` | 15 pt | SemiBold | Inspector title, dialog titles, page headings |
| `body` | 13 pt | Regular | Everything else, buttons, rows |
| `body-strong` | 13 pt | SemiBold | Emphasis inside body, selected tab |
| `caption` | 11 pt | Regular | Hints, coordinates, HUD, table headers |
| `label` | 10.5 pt | SemiBold, +1.2 spacing, caps | Section labels |
| `mono` | 12 pt | Regular | Tags, part numbers, values that must align |

Numbers that sit in columns are right aligned and use `mono`. Text never goes
below 11 pt except the `label` style, which is caps. At most three styles
appear in one panel.

### 2.3 Space

A 4 pt grid. Names, not numbers:

| Token | pt | Typical use |
| --- | --- | --- |
| `space::XS` | 2 | Icon to its label's baseline nudge, inside chips |
| `space::S` | 4 | Between a label and its field, between icon buttons |
| `space::M` | 8 | Between controls in a row, panel inner padding |
| `space::L` | 12 | Between groups, dialog padding |
| `space::XL` | 16 | Between sections, window margin |
| `space::XXL` | 24 | Around an empty state, dialog sections |

Panels use `M` padding horizontally and `L` between sections. Dialogs use `L`
padding and `XL` between sections. A section is introduced by a `label`, never
a divider plus a heading.

### 2.4 Shape and elevation

| Token | Value | Used for |
| --- | --- | --- |
| `radius::XS` | 2 | Key caps, tags inside rows |
| `radius::CONTROL` | 3 | Buttons, fields, rows |
| `radius::SURFACE` | 6 | Floating panels, menus, popups, cards |
| `radius::DIALOG` | 10 | Modal dialogs |
| `radius::PILL` | full | Status pills, chips |

Elevation is tonal first, shadow second:

| Level | Surface | Shadow | Used for |
| --- | --- | --- | --- |
| 0 | `sunken` | none | Canvas surround, fields, lists |
| 1 | `panel` | none, one hairline | Docked panels |
| 2 | `raised` | soft, 8 pt blur | Dock, inspector, tooltips |
| 3 | `raised` | 20 pt blur | Menus, command field, popups |
| 4 | `raised` + scrim | 32 pt blur | Modal dialogs |

In the light theme the shadows carry more of the separation (surfaces are
close in value); in the dark theme the surface step does.

### 2.5 Density

One setting in Preferences, applied to the whole app. The same layout works at
every step; only metrics change.

| Density | Control height | Item spacing | Text | For |
| --- | --- | --- | --- | --- |
| Compact | 20 pt | 6 x 3 | 12 pt body | Laptops, many panels open |
| Standard (default) | 22 pt | 8 x 5 | 13 pt body | Most people |
| Touch | 36 pt | 12 x 8 | 15 pt body | Tablet and commissioning on site |

### 2.6 Icons

Phosphor, through `novo_theme::icons`, in the groups already defined
(`action`, `tool`, `canvas`, `panel`, `mark`, `status`).

- Sizes: 12 (inside a chip), 14 (inside a row), 16 (buttons, dock), 20
  (empty-state and panel headers), 32 (empty-state illustrations). Always a
  whole number of points; icons are vector glyphs, so they stay sharp at any
  scale.
- Regular weight at rest, Fill weight for the active or selected state of a
  toggle (active tool, pinned, current mode), so state is shape and colour, not
  colour alone.
- One icon, one meaning, everywhere: the same name for the same action in the
  dock, the command field, menus and tooltips.
- Electrical concepts Phosphor has no glyph for (terminal strip, cable,
  potential, PE, cross reference, contactor) are drawn as small vector paths in
  `icons::custom`, on the same 16 pt grid and 1.5 pt stroke, themed by the same
  text colours.
- An icon on its own always has a tooltip with the name and the key. An icon
  that sits next to a word needs no tooltip.

## 3. States

Every interactive thing has the same six states, drawn the same way:

| State | Look |
| --- | --- |
| Rest | `surface.control` fill, `border.subtle` hairline; frameless controls show only text |
| Hover | `surface.hover` fill, `border.strong` hairline, text one step brighter |
| Pressed | `surface.hover` fill, `accent` hairline |
| Focus (keyboard) | 2 pt `accent` ring, 1 pt outside the control; never removed |
| Selected | `accent.wash` fill and a 2 pt `accent` bar on the left of a row; a tool or mode toggle fills with `accent.wash` and an accent icon; a visibility or filter toggle is neutral (`filter_toggle`) |
| Disabled | `text.tertiary` text and icon, no hover, no pointer change; the reason is in the tooltip |

Accent as a fill is for the single primary action of a dialog (`primary`
button) and nothing else. Destructive actions are a normal button with `error`
text; the confirmation names what is deleted.

## 4. Components

One widget per job, all in `novo_theme::widgets`. Panels compose these and do
not restyle them.

| Widget | Job |
| --- | --- |
| `ui.button`, `ghost_button`, `primary_button`, `danger_button` | Actions. Ghost has no frame until hover (toolbars, row actions); primary is the copper fill, one per dialog |
| `filter_toggle` | A visibility or filter toggle (layers, map, contours, badges): `surface.hover` fill, primary text and an accent check when on; never the copper wash |
| `icon_button`, `icon_toggle`, `tool_button` | Icon only, tooltip required; a toggle fills with the copper wash when on; `tool_button` is the dock's |
| `segmented` (`segment` for one option) | Two to four exclusive options in a pill track |
| `tabs` | Switch a panel's content: text with a 2 pt accent underline under the current one |
| `text_field`, `search_field`, `number_field` | Input on `sunken` with a hairline; search has the magnifier and a clear button; number fields right align and show the unit |
| `select`, `toggle`, `checkbox`, `radio`, `slider` | As egui, restyled to the states above |
| `section_label` | Opens a panel section |
| `row` | List row: control height + 1 pt, optional leading icon, title, trailing meta in `caption`; hover, selected and drag states |
| `table` | Header in `caption` caps on `panel`, 1 hairline per row, no zebra, numbers right aligned, sticky header, the selected row `accent.wash` |
| `kbd` | A keycap: `radius::XS`, `surface.control`, `mono`; used for every shortcut hint |
| `chip`, `issue_pill` | Compact value or state: filter, count, "No issues" |
| `banner` | Inline status in a panel: status icon, one sentence, optional action, status wash |
| `toast` | Transient result at the bottom centre, stacks upward, never over the dock |
| `menu`, `menu_item` | Rows with icon, label, right-aligned `kbd`; section labels between groups |
| `dialog` | Title, body, footer with actions right aligned, primary last, Esc closes |
| `empty_state` | See 6 |
| `card` | Grouped content on `raised` with `radius::SURFACE` |
| `tooltip` | Name, then key, then one line of help at most |
| `lesson_card`, `pointer`, `note`, `lesson_link`, `tip` | Guidance for learning modules, see 10 |

Dialogs are for things outside the drawing loop (settings, issuing a
revision, import). Everything inside the loop is a panel, the command field, or
the canvas. A dialog is 360, 480 or 640 pt wide, never anything else, and has
one primary action.

## 5. The schematic canvas

The canvas follows the same roles. It has its own palette (`CANVAS_DARK`,
`CANVAS_LIGHT`) because drawing colours have extra jobs (wire blue versus
symbol white, potentials), but they are derived from the same family and tested
the same way.

**Sheet and surround.** The sheet is a paper rectangle, visibly lighter than
the surround in the dark theme (about 8 L* steps, today it is 2) and white on a
warm grey surround in the light theme, with a one pixel frame and a soft 24 pt
shadow so the page reads as an object on a desk, not a hole. The surround is
`surface.sunken`.

**Line weights** (screen: never thinner than 1.25 px; zoomed in: true to the
drawing standard, ISO 128 widths):

| Item | Width | Notes |
| --- | --- | --- |
| Symbol outline | 0.35 mm | |
| Wire | 0.35 mm | `info`-family blue, 2:1 from symbols |
| Sheet frame | 0.5 mm | |
| Title block lines | 0.25 mm outer 0.5 mm | |
| Grid | dots, 1 px | Never lines |
| Selection | +2.5 px over the item, with a 3 px halo at 25% | Copper, plus the halo so it survives colour blindness |
| Hover | +1.5 px, `info` | Solid. A proposal is the same hue dashed at 55% |

**Grid.** Minor dots every grid step, major every fifth, both quiet (minor 1.3:1,
major 1.6:1 on the sheet). Dots fade out below 6 px spacing and the major grid
takes over, so zooming out never turns the page into noise. Snap marker is a
quiet cross; the connection point under the pointer gets a ring.

**Page frame and zones.** Column and row markers sit in the margin in `caption`
size at `text.tertiary`, centred in their zone, with a hairline tick at each
boundary (today they are barely visible). The frame never competes with the
drawing.

**Title block.** Printed exactly as the PDF prints it: labels in 6 pt caps
`text.tertiary`, values in 9 pt `text.primary`, outer rule 0.5 mm, inner rules
0.25 mm. At screen zooms where a label would be smaller than 6 px the labels
drop out and only the values show, so the block never turns into grey smudge.

**Text on the canvas** (device tags, terminal numbers, cross references) has a
minimum on-screen size of 8 px; below that it is hidden, not mush.

## 6. Empty states

Every empty surface says what it is for and offers the next step. Pattern:
a 32 pt icon in `text.tertiary`, a `title` line, one sentence in
`text.secondary`, up to two actions (the first is `primary` only if it is the
screen's main action), and the shortcut as a `kbd` under them. Centred, with
`space::XXL` around, at most 360 pt wide. No illustration beyond the icon.

| Surface | Title | Action |
| --- | --- | --- |
| No project open | Start a project | New project (Ctrl+N), Open example, recent projects list |
| Blank page | An empty sheet | "Press Q to place a symbol", fades once anything is drawn |
| Outline with no match | No pages match | Clear filter |
| Issues, none | All clear | "Run checks again" |
| Changes, none | No changes since the last revision | Issue revision |
| Terminals, Cables, Devices drawer, none | Nothing here yet | The command that fills it |
| Parts search, none | No parts found | Import parts |

## 7. Light theme

Light is a first-class theme, not an inversion. The graphite palette becomes
warm paper: panels a touch darker than floating surfaces (which are lifted
white with a shadow), the same copper accent made darker so it stays 4.5:1,
the schematic on a white sheet that matches print. Preferences offers Dark,
Light and System; the choice is per user, applied at once, and remembered.
Everything in sections 2 to 6 holds in both; a screenshot of every panel is
taken in both themes for each migration PR.

## 8. Using the system

- A panel or dialog is built from the widgets in 4 and the spacing in 2.3. If a
  needed widget does not exist, it is added to `novo-theme` first, in a PR that
  includes a screenshot, and then used.
- New colours are never added for a feature. If a feature seems to need one,
  it is a status, a mark or the accent, or the design is reconsidered.
- Pull requests that change how something looks attach before and after
  screenshots (dark and light) taken with `scripts/screenshot.sh`.
- The design reviewer checks against this page and the audit checklist below.

### Audit checklist

For each panel and dialog, in both themes and all three densities:

1. Only tokens and widgets; no raw colour, font size, radius or spacing.
2. At most three type styles; section labels not headings; mono for tags and
   numbers, right aligned in columns.
3. All six states present; keyboard focus visible; every icon-only control has a
   tooltip with its key.
4. Contrast meets 2.1; nothing is communicated by colour alone.
5. Has an empty state (6), and a sensible narrow-width layout (min window
   640 x 400).
6. Spacing on the 4 pt grid; aligned left edges; consistent row heights.
7. One primary action at most; dialogs have the standard three widths and
   footer.

## 9. Delivery plan

1. This page (docs only).
2. `novo-theme`: roles and `Palette` (dark and light), `space`, `radius`,
   elevation, type styles, density, preference storage; widgets list in 4;
   `scripts/screenshot.sh` and a theme gallery example showing every widget
   in every state. The existing token names keep working.
3. Preferences: theme and density choices.
4. Icons: the sizes and fill rule, then `icons::custom`.
5. Migration, one small PR each, with before and after screenshots: top bar,
   start screen and empty states, outline and pages, command field and menus,
   dock and HUD, inspector, drawers and tables, dialogs, toasts and banners,
   Cabinet canvas chrome.
6. Canvas polish with the canvas thread: sheet and surround, line weights,
   selection and hover, grid fade, page frame, title block, `CANVAS_LIGHT`.
7. Lint: a test that fails on a raw colour, `FontId::new`, or a literal radius
   in `novo-app`.
8. Guidance widgets (section 10) for the learning modules (the consumer is
   learning-modules.md §10), once their engine exists; until then the learning thread uses plain `novo_theme` tokens.

### The gallery

`cargo run -p novo-theme --example gallery` shows every token and widget in one
window, with the theme and density switches in its top bar (`GALLERY_THEME` and
`GALLERY_DENSITY` pick the start look). Pictures of both themes at Standard
density: [dark](images/gallery-dark.png), [light](images/gallery-light.png).

## 10. Guidance: lesson cards, pointers, notes and tips

The learning modules (`docs/design/learning-modules.md`) teach by pointing at
the real controls. The design system owns how that looks, so lessons feel like
part of the program and never like an overlay bolted on. Rule of the whole
section: **guidance helps, it never gets in the way**. It never dims the
screen, never blocks a click and never takes a key.

**Lesson card.** A `raised` surface, `radius::SURFACE`, elevation 2, 320 pt
wide, bottom right of the canvas with `space::XL` clear of the edges, above the
status bar and the dock's row (workspaces.md §7). A 2 pt accent progress bar runs along its top edge.
Header: lesson title in `caption` `text.secondary`, "2 of 4" in `caption`
`text.tertiary`, a close `icon_button`. Body: the step title in `body-strong`,
the step text in `body` `text.secondary` (three lines at most), the check state
as an icon before the title (open circle, filled accent dot for the current
step, `success` check for done, never colour alone). Footer: ghost buttons
`Back`, `Show me how`, `Skip step`, and `Restart lesson` as a `caption` link on
the right. A finished step shows a `success` check for 400 ms before the next
opens; with Reduce motion it changes at once.

**Pointer.** A ring on the target that says "look here": 2 pt `accent`, 14 pt
radius around a point target, a rounded rectangle (`radius::SURFACE`, 4 pt
padding) around a control or area, with a soft halo at 25% opacity. It pulses
(halo grows 8 pt and fades, 1.2 s) and holds still under Reduce motion. When the
target is far from the card, a 1.5 pt accent line with an arrowhead runs from
the card edge to the ring. Copper on the canvas also means the selection, so the
ring is always a ring offset 4 pt outside the item (never the item's own
outline), always pulsing, and never touches the item's own look.

**Note.** A numbered marker (18 pt circle, `accent` fill, `on.accent` number in
`caption` semibold) beside its target, with a bubble next to it: `raised`,
`radius::SURFACE`, elevation 2, 240 pt wide at most, `caption` text, a 6 pt tail
toward the marker. Up to three per step, drawn together so a tour of a screen
reads as a numbered set.

**Tip.** A glossary word in a step's text: `text.primary` with a dotted
underline in `text.tertiary`; hover shows the standard tooltip (name, then one
sentence). Same widget as every other term hint in the app.

**Lesson link.** In an empty state or panel footer: a 14 pt graduation cap
icon, "Lesson: Terminal strips" in `caption`, `text.secondary`, turning `info`
under the pointer. At most one per panel.

**Learn window.** A centred panel like the start panel, on `panel`, tracks as
`section_label`s, each lesson a `row` with title, summary in `caption`, level
and minutes as `chip`s, a 20 pt progress ring (accent on `border.subtle`, a
`success` check when complete), and one button: `Start`, `Continue` or `Do
again`. `Coming soon` lessons are `text.tertiary`, not broken. Empty search
shows the standard empty state.
