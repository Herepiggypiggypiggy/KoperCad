# Interface guidelines

How NovoElectric looks and how its screens are laid out. The look lives in
the `novo-theme` crate; this page says how to use it. Owned by the layout and
branding thread; how the work flows (shortcuts, steps, defaults) belongs to
the workflow thread.

![Theme preview](theme-preview.png)

## Principles

1. **Dark, calm, dense.** One dark theme. The drawing is the brightest thing
   on screen; chrome stays grey and quiet so the eye goes to the page.
2. **One accent for interaction.** Blue (`ACCENT`) means "you can act here"
   or "this is selected in a list". Amber (`BRAND`) is the logo and the
   canvas selection, nothing else.
3. **Status colours mean status.** Green, orange and red only for OK,
   warning and error, always with an icon or text, never colour alone.
4. **Tokens, not hex.** Screens use `novo_theme::tokens::*`. A raw colour in
   UI code is a review comment.

## Setup

The app shell calls `novo_theme::apply(&ctx)` once in the creation context.
It installs the Inter font, Phosphor icons and the dark style, and pins the
theme to dark. The System, Light and Dark choices in the View menu go away
until a light theme exists.

## Tokens

| Token | Hex | Use |
| --- | --- | --- |
| `BG_0` | `#101216` | Text fields, lists, around the sheet |
| `BG_1` | `#171A1F` | Panels, menu bar, status bar |
| `BG_2` | `#1E2229` | Windows, dialogs, menus, tooltips |
| `BG_3` | `#292E37` | Buttons and fields at rest |
| `BG_4` | `#343A45` | Hovered controls |
| `BORDER` | `#2C313A` | Panel and window hairlines |
| `BORDER_STRONG` | `#464D5A` | Hovered or focused outlines |
| `TEXT` | `#E4E7EC` | Body text |
| `TEXT_MUTED` | `#9CA4B0` | Hints, units, headers |
| `TEXT_FAINT` | `#6E7784` | Disabled, placeholders |
| `ACCENT` | `#4C9EFF` | Focus, selection in lists, primary button, links |
| `BRAND` | `#FFB81C` | Logo, canvas selection |
| `SUCCESS` | `#3FB950` | OK |
| `WARNING` | `#F09A3E` | Warning |
| `ERROR` | `#F85149` | Error |

Text sizes: body and buttons 13.5 pt, small 11.5 pt, headings 17 pt
(Inter SemiBold), monospace 12.5 pt for device tags and part numbers.
Controls are 24 pt high. Corners: 4 pt on controls, 8 pt on windows and menus.

Unit tests hold body text to WCAG AA (contrast 7:1 for `TEXT`, 4.5:1 for
`TEXT_MUTED` and the status colours) on every surface.

## Main window

```
+--------------------------------------------------------------------+
| File  Edit  View  Insert  Project  Tools  Help                     |  menu bar
| [save undo redo] | [select wire symbol terminal text] | [snap grid] |  toolbar
+-----------+----------------------------------------+---------------+
| Project   | 2 Power supply | 3 Motor starter  (+)  | Properties    |
|  Pages    |                                        |  Tag   -K1    |
|  Devices  |              canvas                    |  Part  ...    |
|  Parts    |                                        |               |
|  Reports  |                                        |               |
|           +----------------------------------------+               |
|           | Messages (collapsible)                 |               |
+-----------+----------------------------------------+---------------+
| X 125.0  Y 80.0 mm | Grid 2.5 mm | Snap on | 2 selected | No errors |  status bar
+--------------------------------------------------------------------+
```

- **Left, Project (240 pt):** one tree with Pages, Devices, Parts and
  Reports. Pages carry their number and title.
- **Centre:** page tabs over the canvas, with a `+` tab for a new page.
- **Right, Properties (280 pt):** a two column grid, label left and value
  right, for whatever is selected.
- **Bottom of the centre, Messages:** design check results; collapsed until
  there is something to say.
- **Status bar:** cursor position in mm, grid, snap, selection count, and
  the design check summary with its icon.

Panels resize and collapse; the canvas keeps the rest.

## Menus

Proposed top level, in this order. Every item also lives in the command
palette (Ctrl+K), so menus stay short and only list what people look for.

| Menu | Items |
| --- | --- |
| File | New project, Open, Open recent, Save, Save as, Close / Export PDF, Print / Project properties / Quit |
| Edit | Undo, Redo / Cut, Copy, Paste, Duplicate, Delete / Select all / Find |
| View | Command palette / Zoom to fit, Zoom to selection / Grid, Snap / Panels: Project, Properties, Messages |
| Insert | Symbol, Wire, Terminal, Cable, Cross reference, Text / Page |
| Project | Pages, Devices, Parts / Reports / Check design |
| Tools | Simulate / Scripts / Settings |
| Help | Keyboard shortcuts, About NovoElectric |

Rules: items are verbs or nouns in sentence case; `...` only when a dialog
follows; shortcuts show on the right; disabled rather than hidden when they
do not apply; separators (`/` above) group items, at most four groups.

## Icons

Phosphor Regular, through `novo_theme::icons`. Use the names in
`icons::action`, `icons::panel` and `icons::status` so the same action always
has the same icon in menus, the toolbar and the command palette. Menu items
show the icon before the label; toolbar buttons show the icon only, with the
label and shortcut in the tooltip.

## Canvas

`novo_theme::CANVAS_DARK` is the proposed screen palette for `novo-canvas`
(WS-03 owns rendering): light grey symbols and blue-grey wires on a near black sheet,
amber selection, blue hover, green open connection points, blue cross
references. Printing and PDF stay black on white.

The app maps it onto `novo_canvas::widget::CanvasStyle` (background, grid,
selection, hover, rubber band, snap marker) and `novo_canvas::model::ModelStyle`
(symbol, wire, tag text, sheet fill, frame) and applies it with
`PageScene::set_style`. novo-canvas itself keeps black on white defaults.

## Branding

- Name: NovoElectric, one word, capital N and E.
- Mark: an "N" drawn as an open contact (`assets/branding/icon.svg`).
- Tagline: "Electrical design, without the weight".
- Window icon and installer icon come from `assets/branding`; see its README.
