# Visual redesign addendum: 3D overlays and panels

Graphic design thread, 2026-10-28. Addendum to `visual-redesign.md` (sections 3.2 and 8 and lane V3b). It answers the owner's complaint of 2026-10-28 about the 3D view:

- EMC cannot be turned off in the 3D configuration panel.
- The configuration panel is frustrating in general.
- The other panels that can appear (Inspector, Temperature, Door, Sizing and so on) are separate popups that are hard to find, cannot be closed, and are not easy to open again.
- Buttons are doubled: turning heat off in the 3D panel leaves it on in the top bar.

The board `visual-redesign/images/workspace-3d-dark.png` shows the target look (3D rail with a Layers flyout of switches). This page adds what the board does not say: **who owns each state, and where each panel lives.**

## 1. The causes this page designs against

These are read from the owner's words and the board; the 3D thread must confirm each in the code before it relies on it.

1. A toggle exists in more than one place and each place keeps its own state (the heat switch in the top bar and the heat switch in the 3D panel).
2. A switch that cannot be turned off is usually a switch whose state is recomputed from something else every frame; the owner of the state is not the switch.
3. Each overlay opened its own popup, so there is no list of what can be open and no single way to close them.

## 2. Rules

1. **One state, one owner.** Every 3D view option (a layer or an overlay) is one entry in one registry with one stored value. The rail button, the Layers flyout row, the Ctrl K command, a key and the menu row all read and write that entry. No surface keeps a copy.
2. **The top bar carries no view toggles.** It holds the mark, undo and redo, the workspace segment, the page, search and issues (visual-redesign.md section 3.1). Heat, EMC and layers never appear there. The existing top-bar heat button is removed in the same change that introduces the registry.
3. **A toggle appears once on the rail side.** Either it is an overlay with its own panel and gets a rail button (Heat, EMC view, Wire runs), or it is a plain visibility layer and is a row in the Layers flyout (Enclosure, Door, Wires, Ducts, Shadows, Ambient occlusion, Dimensions). The same option is never both.
4. **Off means off.** Turning an overlay off hides its drawing, its legend and its tab, and survives selection changes, a new run and reopening the project. A test turns each option on and off from every surface and asserts the same value everywhere.
5. **Every panel can be closed and found again.** Every panel has the host header (icon, title, dock left, dock right, close), `Esc` closes the topmost floating panel, and a **Panels** entry on the rail lists every panel with its state (open or closed) so a closed one is one click away. Ctrl K has "Show <panel>" for each.
6. **Fewer panels, not more windows.** The seven popups become one right-hand panel group with tabs.

## 3. What lives where

| Today (owner's words) | Becomes | Where it opens |
| --- | --- | --- |
| 3D configuration panel | The **Layers flyout** (switches only) and the rail toggles below | Rail: Layers |
| Heat in the top bar and in the panel | One option `heat` | Rail: Heat toggles the overlay and shows the **Heat** tab |
| Temperature | The **Heat** tab: scale and legend, hottest device, total loss | Right panel group |
| EMC | One option `emc`; the **EMC** tab holds the legend and the rules list | Rail: EMC view; right panel group |
| Inspector | The **Inspector** tab; it follows the selection | Opens on selection; right panel group |
| Door | A section of the Inspector when a door is selected (angle slider); the Layers row "Door" keeps showing the angle | Inspector |
| Sizing | A section of the Inspector when the enclosure is selected; the sizing tool is "Size enclosure" on the Cabinet rail | Inspector, Cabinet rail |

The right panel group is the one right slot of direction.md section 6: one panel at a time with tabs (Inspector, Heat, EMC), 280 wide by default, resizable, dockable left or right, floatable as a whole, remembered per workspace layout (panel rules in visual-redesign.md section 8). Opening a tab from the rail opens the group if it is closed and selects the tab.

## 4. Tests

- **Single state:** for each registered option, set it from the rail, the Layers flyout, Ctrl K and the key; assert all surfaces report the same value and that the drawing agrees.
- **No duplicates:** a test lists the options each surface shows and fails if an option appears in two of the rail-side places (rule 3), or in the top bar (rule 2).
- **Closable:** every panel in the registry has a close action and a "Show" command, and closing then showing restores the dock slot and size.
- **Off survives:** turn EMC and Heat off, select another device, rerun, reopen the project; they stay off.

## 5. Order of work and lane V3b

Lane V3b (the 3D rail) should be **built together with the 3D rework**: they are the same surface. Suggested order inside one thread, each step its own PR:

1. **Registry and top bar** (needs nothing from the redesign): one `ViewOption` registry, rule 4's test, the top-bar heat button removed, EMC fixed. This ships on today's UI and fixes the bugs first.
2. **Panel group** (uses today's panel helper; gets docking for free when lane V1 lands): the Inspector, Heat and EMC tabs with close, Esc and the Panels entry; Door and Sizing become Inspector sections.
3. **3D rail and flyouts** (needs lane V3a, the rail core): the rail of the 3D board, the Layers flyout, the Panels entry; the 43 View > 3D menu rows leave the menu (visual-redesign.md section 5.2).

## 6. Honest limits

The board is an HTML picture, not egui output. The three causes in section 1 are inferred from the owner's description, not yet traced in the code. "Door" and "Sizing" as Inspector sections is a design proposal: if the 3D thread finds that either needs to stay a tool (it edits geometry rather than showing properties), it stays a rail tool and the Inspector shows only the read-only values.
