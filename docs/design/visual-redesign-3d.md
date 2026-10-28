# Visual redesign addendum: 3D overlays and the side panel

Graphic design thread, 2026-10-28. Addendum to `visual-redesign.md` (sections 3.2 and 8 and lane V3b). It answers the owner's complaints of 2026-10-28 about the 3D view:

- EMC cannot be turned off in the 3D configuration panel.
- The configuration panel is frustrating in general.
- The other panels that can appear (Inspector, Temperature, Door, Sizing and so on) are separate popups that are hard to find, cannot be closed, and are not easy to open again.
- Buttons are doubled: turning heat off in the 3D panel leaves it on in the top bar.
- "Vents and fans" and "Heat" overlap each other. The owner's proposal: **one long panel on the right with individually foldable sections, each of which can be undocked into a floating window.** This page adopts that model.

![3D side panel](visual-redesign/images/side-panel-dark.png)

| Board | Dark | Light |
| --- | --- | --- |
| 3D side panel: foldable sections, Heat undocked | [png](visual-redesign/images/side-panel-dark.png) | [png](visual-redesign/images/side-panel-light.png) |

The board `workspace-3d` still shows the earlier Layers flyout; the side-panel board supersedes it.

## 1. The causes this page designs against

Confirmed in the code by the 3D rework thread: `view3d.rs` sets the EMC state from the controls before the toolbar runs and writes it back after the panels, which undoes a click; Cabinet has its own EMC state; Heat has three owners (the controls, the Cabinet heat state and the 3D heat layers). Inferred from the owner's words: each overlay opened its own popup, so there is no list of what can be open and no single way to close them.

## 2. Rules

1. **One state, one owner.** Every 3D view option (a layer or an overlay) is one entry in one registry with one stored value. The section switch, a Layers row, a rail button, the Ctrl K command and a key all read and write that entry. No surface keeps a copy, and nothing recomputes the value from somewhere else each frame.
2. **The top bar carries no view toggles.** It holds the mark, undo and redo, the workspace segment, the page, search and issues (visual-redesign.md section 3.1). The existing top-bar heat button is removed in the same change that introduces the registry.
3. **The switch lives in the section header.** An overlay that has a section (Heat, Vents and fans, EMC) is switched on and off by the switch in its header, **written as the word On or Off next to the switch**, never by colour alone. Plain visibility layers are rows of the Layers section, with On or Off written at the right. The rail buttons for these do not switch anything: they open the side panel and unfold the section (section 4).
4. **Off means off.** Turning an overlay off hides its drawing and its legend, and survives selection changes, a new run and reopening the project. A test turns each option on and off from every surface and asserts the same value everywhere.
5. **Every section can be folded, undocked, closed and found again.** Section 3 says how.
6. **Keyboard and words.** Section headers, the switches and the Layers rows are reachable with Tab and the arrow keys; Enter or Space folds a section or flips a switch; Esc closes the topmost floating section. A switch is never colour only.

## 3. The side panel

One panel on the right (280 to 300 wide by default, resizable, dockable left or right, floatable as a whole, remembered per workspace layout; panel rules in visual-redesign.md section 8). Its sections, in the default order:

| Section | What it holds | Switch | Shown |
| --- | --- | --- | --- |
| Inspector | The selection: device, position, rail, clearance. Follows the selection | none | Always |
| Layers | Visibility rows: Enclosure, Door, Wires, Ducts, Shadows, Ambient occlusion, Dimensions, each with On or Off written. The Door row is visibility only; the angle lives in the Door section, so it is stored once | none (rows) | Always |
| Heat | The scale and legend, hottest device, total loss, run. Replaces the separate "Temperature" popup | On or Off | Always |
| Vents and fans | The cooling inputs (vents, fans, flow) that the Heat results depend on. A separate section because the owner asked for it; both read the same heat state | On or Off | Always |
| EMC | The legend and the rules list | On or Off | Always |
| Door | Door angle and clearance swing | none | When there is an enclosure |
| Sizing | Enclosure size read-outs; the sizing tool stays "Size enclosure" on the Cabinet rail | none | When there is an enclosure |

Behaviour of each section header (left to right: fold chevron, icon, title, switch where it has one, undock button):

- **Fold.** Click the header or press Enter. Several sections can be open at once; the column scrolls and the headers of open sections stick to the top while their body scrolls. Alt+click folds all the others (solo). Fold state is remembered.
- **Undock.** The undock button, or dragging the header out of the panel, turns the section into a floating window with the standard host header (icon, title, switch, dock back, close), resizable, remembered. The row stays in the list as a dimmed "Floating" row with a dock-back button, so nothing disappears from the panel. Dragging the floating window to the panel's edge docks it back; the dock-back button returns it to its place in the order.
- **Close.** Closing a section hides it. The dots menu in the panel header ("Sections") lists every section with its state, and Ctrl K has "Show <section>" for each, so a closed section is one click away. Closing a section never changes its On or Off value.
- **Esc** closes the topmost floating section; the panel itself closes with its X, the rail "Side panel" button, or Ctrl K.

The **same component** can serve the Cabinet workspace (its own section list, for example Inspector, Heat, Vents and fans, EMC on the plate, Placement rules, Cabinet table). It is a section list per workspace, a data table like the rail tables, so a new section adds one row.

## 4. The rail on the 3D workspace

The 3D rail (visual-redesign.md section 3.2) keeps its tools and assists. Its panels zone has four places: **Side panel** (show or hide the whole panel), **Layers**, **Heat** and **EMC**. The last three open the side panel if it is closed and unfold their section; they do not switch an overlay (rule 3). The zone stays at 4 base places, so the add-on slot (visual-redesign-addons.md section 3) fits as the fifth. "Build order" moves to the assists zone (5). "Wire runs" is the Wires row of Layers.

## 5. Tests

- **Single state:** for each registered option, set it from the section switch or row, Ctrl K and the key; assert all surfaces report the same value and that the drawing agrees.
- **No duplicates:** a test lists the options each surface shows and fails if an option has a second owner or appears in the top bar (rule 2).
- **Foldable, undockable, closable:** every section has fold, undock and close; undock then dock back restores the order; close then "Show" restores the section and its fold state.
- **Off survives:** turn EMC and Heat off, select another device, rerun, reopen the project; they stay off.
- **Keyboard:** the headers, switches and Layers rows are reachable and operable without the mouse.

## 6. Order of work and lane V3b

Lane V3b (the 3D rail) is **built together with the 3D rework**, as one thread. Suggested order, each step its own PR:

1. **Registry and top bar** (needs nothing from the redesign): the view-option registry, rule 4's test, the top-bar heat button removed, EMC fixed. This ships on today's UI.
2. **Side panel** (today's panel helper; docking arrives with lane V1): the section column with fold, undock, close and the Sections menu; Inspector, Layers, Heat, Vents and fans, EMC, Door and Sizing as sections.
3. **3D rail** (needs lane V3a, the rail core): the rail of the board, with the panels zone of section 4.

## 7. Honest limits

The board is an HTML picture, not egui output. Dragging a header out of a panel to undock it is custom code on top of egui areas; lane V1 must prove it with scripted drag tests, and the undock button is the fallback if the drag proves unreliable. "Temperature" is merged into Heat on the assumption that they show the same data; the 3D thread confirms. Door and Sizing as top-level sections follow the owner's list; if either edits geometry rather than showing properties, it stays a tool and its section shows read-only values.
