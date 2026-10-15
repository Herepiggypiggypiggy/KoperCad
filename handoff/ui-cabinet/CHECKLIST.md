# Cabinet features versus where the user finds them

Status key: UI = reachable in the app today, ENGINE = engine only (on main or in an open PR), no way to reach it yet.
"Where" is the place the user should find it; empty means not decided or not built. Updated 2026-10-15 from PR titles; verify against each PR before closing a row.

| Feature | PR / state | Status | Where the user finds it |
| --- | --- | --- | --- |
| Plate, rails, placements canvas | #372 merged | UI | Ctrl+2 Cabinet |
| Unplaced list, drag to place, landing box | #374 #376 #378 merged | UI | Left panel of the Cabinet |
| Place all preview, Enter accepts | merged | UI | Header strip |
| Plate size (width, height, margin) | this thread | UI (PR 1) | Inspector, nothing selected |
| Device data, part, problems | this thread | UI (PR 1) | Inspector, device selected |
| Table face and problem hover text | #403 open | UI pending merge | Ctrl+T on the Cabinet |
| Plate / Table / 3D face switch | design in #395 | ENGINE | Header strip, three faces |
| Enclosure inside, depth, door swing check | #409 open | ENGINE | Inspector (enclosure section) and problems list |
| Power loss, mounting type, per-side clearances | #419 open | ENGINE | Inspector, part data section |
| Enclosure sizing helper | #402 open | ENGINE | Dialog from the Cabinet header: "Size the enclosure" |
| Heat map, hot-device ranking, loss bridge | #406 open | ENGINE | Overlay toggle with legend in the header; ranking in the inspector |
| Ducts in the model, duct fill | #362 #358 merged | ENGINE (not drawn) | Ducts drawn on the plate, fill in the inspector |
| Route connections, wire lengths | #381 merged, wire list length in progress | ENGINE | Routes toggle on the plate; length column in the wire list |
| Duct checks, unplaced devices check | #413 open | ENGINE | Problems list on the Cabinet and Issues in the outline |
| Plate drawing and drill table (R5) | #397 open | ENGINE | Export plate drawing, from the Cabinet header and Ctrl+K |
| 3D scene from cabinet data | #391 open | ENGINE | 3D face |
| 3D look and camera | #420 open | ENGINE | 3D face |
| 3D view cube, view.* commands, 3D toolbar, Settings > 3D view (spec docs/design/3d-look.md section 8, engine in #420) | #388 #420 open | ENGINE | View cube top right of the 3D face; toolbar for toggles; commands in the single list; Settings page |
| Part power loss, mounting, clearances fields | #419 open, `cabinet_panel::show` ready | ENGINE | Section inside the inspector (wire after #443 and #419 merge) |
| Inside and door section | #409 open, `DoorState::show` ready | UI pending merge | Collapsible section in the inspector, replacing its own panel |
| 3D renderer (novo-view3d) | spike after #395 | ENGINE | 3D face |

## Order of work (small draft PRs, no build)

1. Inspector with plate size and device data (done, PR 1).
2. Problems list strip under the header (counts become a clickable list that selects the device). Needs nothing unmerged.
3. Faces switch in the header, with Plate and Table; 3D is greyed out with a hover note until novo-view3d lands. After #403.
4. Enclosure and part data in the inspector after #409 and #419 merge.
5. Heat overlay toggle with legend after #406; duct and route drawing after the canvas exposes it.
6. Sizing helper dialog after #402; plate drawing export after #397.
7. 3D face after the novo-view3d spike.

## Coordination

Layout and look: inspector uses the same panel pattern as the Terminals drawer; the Layout thread reviews spacing. Workspaces thread: Ctrl+2 and the faces switch must not collide with its tabs. 3D threads: the face switch calls one function, `show_3d(ui, structure)`, they supply it.
