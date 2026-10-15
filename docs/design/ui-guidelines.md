# Interface guidelines

How KoperCAD looks: the visual language. The look lives in
the `novo-theme` crate; this page says how to use it. Owned by the layout and
branding thread; how the work flows (shortcuts, steps, defaults) belongs to
the workflow thread.

![Theme preview: the Workbench window](theme-preview.png)

The preview (`cargo run -p novo-theme --example preview`) is the Workbench
window from `docs/DESIGN.md` §3, drawn with these tokens and widgets.

## Principles

1. **Graphite, calm, dense.** One dark theme of near-neutral greys with a
   faint warm cast. The drawing is the brightest thing on screen; chrome
   stays quiet so the eye goes to the page. Hierarchy comes from value and
   spacing, not colour.
2. **One accent, copper, used rarely.** Copper (the conductor) marks focus,
   the active tool, the selection on the canvas, the primary button and the
   logo, plus the conductor joints and open wire ends on the canvas (the
   same copper dot as the logo). Nothing else is copper, so it always means
   "here". Copper is never a mark: proposals, problems, overrides and changes use other colours, so
   the selection is always unambiguous (DESIGN §5).
3. **Status colours mean status.** Muted green, amber and red only for OK,
   warning and error, always with an icon or text, never colour alone.
4. **Labels, not headlines.** Panels open with a small spaced uppercase
   section label (`widgets::section_label`), not a large bold heading.
5. **Tokens, not hex.** Screens use `novo_theme::tokens::*` and
   `novo_theme::widgets::*`. A raw colour in UI code is a review comment.

## Setup

The app shell calls `novo_theme::apply(&ctx)` once in the creation context.
It installs the Inter font, Phosphor icons and the dark style, and pins the
theme to dark. Any light, dark or system theme choice goes away
until a light theme exists.

## Tokens

| Token | Hex | Use |
| --- | --- | --- |
| `BG_0` | `#0E0E10` | Text fields, lists, around the sheet |
| `BG_1` | `#151517` | Top bar, outline, drawer |
| `BG_2` | `#1B1B1E` | Floating surfaces: inspector, dock, command field, tooltips |
| `BG_3` | `#232327` | Buttons and fields at rest |
| `BG_4` | `#2D2D32` | Hovered controls |
| `BORDER` | `#252529` | Hairlines |
| `BORDER_STRONG` | `#3B3B41` | Hovered or focused outlines |
| `TEXT` | `#E3E1DC` | Body text |
| `TEXT_MUTED` | `#989691` | Hints, units, section labels |
| `TEXT_FAINT` | `#858380` | Placeholders, coordinates, margin labels, disabled |
| `ACCENT` | `#C88A5E` | Copper: focus, active tool, canvas selection, primary button, logo |
| `ACCENT_SELECTION` | `#3A2E26` | Selected rows and selected text |
| `INFO` | `#8FA8C8` | Links, cross references, hover on the canvas |
| `SUCCESS` | `#7FB086` | OK |
| `WARNING` | `#D6A55C` | Warning |
| `ERROR` | `#E0766C` | Error |

Text sizes: body and buttons 13 pt, small 11 pt, headings 15 pt (Inter
SemiBold, for the inspector title and document pages such as Rules),
monospace 12 pt for device tags and part numbers. Controls are 22 pt high.
Corners: 3 pt on controls, 6 pt on floating surfaces.

Building blocks in `novo_theme::widgets`:
- `section_label` for panel and inspector sections.
- `segment` for two- or three-way switches (Schematic | Cabinet, outline
  modes, drawer tabs).
- `tool_button` (frameless icon, copper with an underline when active) for
  the dock.
- `issue_pill` for the issue count in the top bar, the only place it shows.
- `field` for a read-only label and value row in the inspector.
- `primary_button` (copper text and hairline, no solid fill) for the one
  main action of a project-level screen or a command preview ("Place 12
  devices"). The inspector edits live, so it has no Apply button. Dialogs
  are for things outside the drawing loop, never the default.

Unit tests hold every text colour to a contrast floor, so no text the user
has to read is decorative:

| Text | Minimum contrast | On |
| --- | --- | --- |
| `TEXT` | 7:1 | every surface |
| `TEXT_MUTED` | 4.5:1 | every surface |
| `TEXT_FAINT` | 4.5:1 | `BG_0` to `BG_2` (canvas, panels, cards) |
| Accent and status colours | 4.5:1 | `BG_0` to `BG_2` |
| Drawing colours | 4.5:1 | the sheet (`CANVAS_DARK`) |

Faint text is not for disabled-looking decoration: the HUD, page numbers in
the outline and column headers all use it, so it has to be readable.

## Layout and commands

Window structure, panels and how commands work are in `docs/DESIGN.md`.
This file covers only the visual language those screens use.

## Icons

Phosphor Regular, through `novo_theme::icons`. The modules follow the
design's vocabulary: `action` (commands), `tool` (the four dock tools),
`canvas` (Schematic, Cabinet), `panel` (outline modes, inspector, drawer
tabs), `mark` (proposal, override) and `status`. Use these names so the same
thing always has the same icon. Rows in the command field show the icon
before the label and the key after it; dock buttons show the icon only, with
the label and key in the tooltip.

## Canvas

`novo_theme::CANVAS_DARK` is the proposed screen palette for `novo-canvas`
(WS-03 owns rendering): warm light grey symbols and blue-grey wires on a
near black sheet, bright copper selection, steel blue hover and cross
references, muted green open connection points. Printing and PDF stay black on white.
The DESIGN.md §5 marks are in the palette too: proposals in the hover hue
at 55% (dashed, where hover is solid), a grey override diamond, and change
marks in the status colours at 50%. A test keeps every mark off copper.

Links (`docs/design/canvas-feedback.png`): wires are a mid saturated blue
`#6F9BDB` and symbols near white `#ECEAE5`, at least 2:1 apart, so a wire
never merges into a terminal outline. A filled copper dot (`junction`,
radius 3.5 px at 100% zoom) marks a true junction only: three or more
conductors joining, as DESIGN §2 defines it (IEC 60617: no dot on a
straight run, corner, crossing or a wire ending on a pin). Joints are not
marks (marks are proposals, problems, overrides and changes), and the
selection never relies on colour alone: it is also 2.5 px thicker with a
halo, and junctions are fixed-size dots, so a selected net stays
unmistakable without colour vision. Pins: a free pin is a hollow ring
(open-connection green, r 3), a connected pin a small filled dot in the wire
colour (r 2.5), so a pin end never looks like a junction. A wire ended
unconnected (DESIGN §4, the `wire.open` Warning) gets an open copper ring
(`wire_end`, r 4, 1.6 px line) and an amber flag (`unconnected_flag`, 16 px
pole, 10 px pennant) with the tooltip "Unconnected". The radial place picker (five wedges: Terminal,
Switch, Relay, Breaker, Points around an Other... centre; inner radius 38, outer 110, hovered wedge
copper outline, its items as pills on an outer ring) and the
right-click menu (26 px rows, 236 px wide, shortcut right-aligned) are in
the same mock-up; behaviour is Workflow's spec.

The app maps it onto `novo_canvas::widget::CanvasStyle` (background, grid,
selection, hover, rubber band, snap marker) and `novo_canvas::model::ModelStyle`
(symbol, wire, tag text, sheet fill, frame) and applies it with
`PageScene::set_style`. novo-canvas itself keeps black on white defaults.

## Branding

- Name: KoperCAD, one word, capitals K C A D. Made by KoperVonk ("by KoperVonk"
  in the About box and installer; `assets/branding/lockup.svg` is the lockup).
  Formerly NovoElectric.
- Mark: "Nova", a four-ray star meeting at a copper junction dot
  (`assets/branding/icon.svg`).
- Tagline: "Electrical design, without the weight".
- Window icon and installer icon come from `assets/branding`; see its README.

## Label placement

One rule for every symbol: the device tag, terminal number and potential
feed name sit to the left of the symbol, right-aligned and about 3 mm clear,
centred on the symbol (145 of 151 library symbols already do; terminals and
potential feeds had disagreed). A terminal strip's own name sits left of the
run, clear of the terminals' labels. Only the semiconductors with a control
terminal on the left (thyristor, triac, transistors) keep their tag on the
right, so it does not cross the gate.

## Terminals, potentials and the pages navigator

Mock-up: `docs/design/workflow-looks.png`.

- **Connected terminals.** Terminals joined into a group keep their circles
  and leads and get a bridge: a heavy bar (about 0.5 mm) across the upper
  leads, 2.6 mm above the circle centres (y 1.4 in terminal.toml), with a
  filled dot of about 0.4 mm radius where it meets each lead. The bar stays
  clear of the numbers on the left. Plain terminals have one circle and one
  lead pair, as before.
- **Potential menu** (right-click a feed): a "Potential" caption, a 4 by 2
  grid of chips (L1, L2, L3, N, PE, +24 V, 0 V, New...) 55 by 28 px, the
  current one with a copper outline on BG_3; then "Number automatically",
  "Rename..." (F2), "Show where it goes", and "Delete". One click on a chip
  sets the potential of all selected feeds and closes the menu; "New..."
  creates a project potential.
- **Pages navigator** (the outline's Pages section, 248 px wide, Function
  view shown; the mock-up is wider and titled PAGES for legibility): rows 24 px high, group (=name),
  panel (+name) and page rows indented 16 px per level, caret to fold, page
  count at the right, the open page with a copper edge on BG_3. A panel name
  that occurs under more than one group gets a link icon in INFO and a
  footnote ("appears in two groups: one shared layout"). "+" in the header
  and on a row's context menu adds a page group, panel or page.

