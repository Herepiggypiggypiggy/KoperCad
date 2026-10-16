# Cabinet toolbar

Status: proposal for design review. Mock: `cabinet-toolbar/mock.html`
(pictures `mock-dark.png`, `mock-light.png` beside it). Parent spec:
`workspaces.md` section 3.2, tokens in `crates/novo-theme`.

## Problem

The Cabinet header is one row of 14 equal, text-only controls at the default
button size, mixed with status text. The main action, **Place all**, looks like
Plate, Table and Cut list. A lesson says "Click Place all" and the person
cannot find it. Heat adds more chips into the same row, and the Rails and
ducts button (#612) would add another. It does not scale.

## Design

One bar, 56 px high, controls 28 px, text 14 px, icons 16 px (`icon_size::BUTTON`),
every button icon plus label. Buttons sit in named groups in workflow order,
each group with a small-caps caption above it (`text_tertiary`, which the Palette guarantees 4.5:1; captions are 12 px at minimum, the mock draws them at 12 px):

| Group | Contents | Notes |
| --- | --- | --- |
| Panel | panel picker; Create cabinet and Set plate when the panel has none | left edge |
| Place | **Place all** (primary), Rules | |
| Arrange | Plate setup (size and fill), Size enclosure | |
| Route | Route all, Rails and ducts, Cut list | Rails and ducts arrives with #612 |
| View | Plate / Table segmented switch (the face), Heat toggle, Door | |
| Output | Export menu: Plate drawing (PDF), later DXF | right edge |

A status strip below carries the counts as pills (`0 placed`, `5 unplaced`,
`2 problems` with the existing problems menu, `n rule problems`). Counts leave
the toolbar, so the bar holds only things you press.

### Place all

- The only copper (`primary_button`) control on the bar. Icon, label, and the
  shortcut shown as a `kbd` chip. New command `cabinet.place-all`, default key
  **Ctrl+Shift+P** (Anywhere in the Cabinet workspace; Shift+P is `view.toggle_marks`, so it is not used), searchable in the command field as
  "place all"; menu: Cabinet > Place all.
- Label says what it does when it is cheap to know: "Place all" stays the
  name (the lessons and the command use it); the tooltip says "Place the 5
  unplaced devices on the DIN rail, you review before it is stored".
- When everything is placed it is greyed, and it keeps its position so the
  bar does not jump. The reason is written in the status strip ("Every device
  of this panel is placed"), not only in the tooltip, so keyboard users see it.
- While a preview is shown, the Place group becomes **Accept (Enter)**
  (primary) and **Cancel (Esc)**, plus **Why here?** (opens the rule
  explanation that exists today). Place all steps aside; nothing else changes
  position.
- Lesson anchor id `cabinet.place-all`, so a step can point at it.

### Progressive disclosure

- Heat controls (Map, Contours, Airflow, Badges, Simulate, status) live in a
  second strip that appears only while Heat is on. The main bar never grows.
- Below 1100 px the group captions drop, Arrange, Rails and ducts, Cut list,
  Rules and Export fold into a **More** menu; Place all, Route all, the panel
  picker and the Plate/Table switch always stay. Labels never truncate: a
  group that does not fit folds whole.
- Door stays in View at wide widths and folds into More with its group. Order of folding, last to first: Output, View's Heat and Door, Route's Rails and ducts and Cut list, Arrange, Rules. Place all, Route all, the panel picker and the Plate/Table switch never fold.
- One rule: labels are never shortened or turned into icon-only buttons; whole groups fold into More.

### Design system fit

All colours are `Palette` roles (panel, control, raised, accent, accent_wash,
text_*, warning and error washes for pills). Toggles on use `accent_wash` with
an accent outline. Pill counts use the Level washes. No new tokens. The new
widgets (`group`, `toolbar_button` with icon, label and kbd, `count_pill`)
go into `novo-app` first; if the Layout thread wants them in `novo-theme`
they move unchanged.

### Key and focus rule

The bar never takes keyboard focus away from the plate: a pointer click on a bar
button returns focus to the canvas, so R, Delete and moving a device keep working
straight after Place all and Accept. (The bug that F1 cannot be clicked and
moved after Place all is owned by the Cabinet key-bugs thread; this design
only promises the bar is not the cause.) The rule is for pointer clicks only: Tab and Enter on a button, and opening a menu or the panel picker, keep focus where it is.

## Where the user finds it

Ctrl+2 (Cabinet), then the bar across the top of the plate. Place all is the
copper button on the left of the bar, or Ctrl+Shift+P, or the command field.

## Build plan (small PRs, in this order)

1. **Bar structure**: groups, icons, 28 px controls, status strip, Place all
   primary with command and key, anchors. Same actions, new layout.
2. **Responsive and More**: folding by width, Export menu.
3. **Heat strip**: move `heat::chips` into the second strip (coordinate with
   CFD-F).
4. **Lessons**: labels and anchors in the lesson files match this note.
   Rails and ducts (#612) and the surface picker (PANEL-B3) drop into the
   Route and Panel groups after those PRs merge.

## Open questions for the reviewers

- Ctrl+Shift+P as the key for Place all: any conflict in the keymap?
- Group captions: kept always while they fit; they drop below 1100 px (answered by the design reviewer).
