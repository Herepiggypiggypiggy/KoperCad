# Interface guidelines

How NovoElectric looks: the visual language. The look lives in
the `novo-theme` crate; this page says how to use it. Owned by the layout and
branding thread; how the work flows (shortcuts, steps, defaults) belongs to
the workflow thread.

![Theme preview](theme-preview.png)

## Principles

1. **Graphite, calm, dense.** One dark theme of near-neutral greys with a
   faint warm cast. The drawing is the brightest thing on screen; chrome
   stays quiet so the eye goes to the page. Hierarchy comes from value and
   spacing, not colour.
2. **One accent, copper, used rarely.** Copper (the conductor) marks focus,
   the active tool, the primary button and the logo. Nothing else is copper,
   so it always means "here".
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
| `BG_1` | `#151517` | Panels and bars |
| `BG_2` | `#1B1B1E` | Windows, dialogs, menus, tooltips |
| `BG_3` | `#232327` | Buttons and fields at rest |
| `BG_4` | `#2D2D32` | Hovered controls |
| `BORDER` | `#252529` | Hairlines |
| `BORDER_STRONG` | `#3B3B41` | Hovered or focused outlines |
| `TEXT` | `#E3E1DC` | Body text |
| `TEXT_MUTED` | `#989691` | Hints, units, section labels |
| `TEXT_FAINT` | `#858380` | Placeholders, coordinates, margin labels, disabled |
| `ACCENT` | `#C88A5E` | Copper: focus, active tool, primary button, logo |
| `ACCENT_SELECTION` | `#3A2E26` | Selected rows and selected text |
| `INFO` | `#8FA8C8` | Links, cross references, hover on the canvas |
| `SUCCESS` | `#7FB086` | OK |
| `WARNING` | `#D6A55C` | Warning |
| `ERROR` | `#E0766C` | Error |

Text sizes: body and buttons 13 pt, small 11 pt, dialog titles 15 pt
(Inter SemiBold), monospace 12 pt for device tags and part numbers.
Controls are 22 pt high. Corners: 3 pt on controls, 6 pt on windows and menus.

Building blocks in `novo_theme::widgets`: `section_label` for panel
sections, `primary_button` (copper text and hairline, no solid fill) for the
one main action in a dialog, and `tool_button` (frameless icon, copper with
an underline when active) for the toolbar.

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

## Layout and menus

Window structure, panels and how commands are organised are being designed
in `docs/design/screen-architecture.md` (layout thread) and
`docs/design/interaction-model.md` (workflow thread), merged into
`docs/DESIGN.md`. This file covers only the visual language those screens
use.

## Icons

Phosphor Regular, through `novo_theme::icons`. Use the names in
`icons::action`, `icons::panel` and `icons::status` so the same action always
has the same icon in menus, the toolbar and the command palette. Menu items
show the icon before the label; toolbar buttons show the icon only, with the
label and shortcut in the tooltip.

## Canvas

`novo_theme::CANVAS_DARK` is the proposed screen palette for `novo-canvas`
(WS-03 owns rendering): warm light grey symbols and blue-grey wires on a
near black sheet, bright copper selection, steel blue hover and cross
references, muted green open connection points. Printing and PDF stay black on white.

The app maps it onto `novo_canvas::widget::CanvasStyle` (background, grid,
selection, hover, rubber band, snap marker) and `novo_canvas::model::ModelStyle`
(symbol, wire, tag text, sheet fill, frame) and applies it with
`PageScene::set_style`. novo-canvas itself keeps black on white defaults.

## Branding

- Name: NovoElectric, one word, capital N and E.
- Mark: "Nova", a four-ray star meeting at a copper junction dot
  (`assets/branding/icon.svg`).
- Tagline: "Electrical design, without the weight".
- Window icon and installer icon come from `assets/branding`; see its README.
