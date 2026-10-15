# Cabinet layout (WS-15)

How KoperCAD places devices on a mounting plate and checks the result. This
extends `docs/DESIGN.md` (Cabinet canvas, drawer, journey step 8); where the
two disagree, DESIGN.md wins and this file is corrected. Engine code lives in
`novo-layout`; the UI stays thin.

## 1. What the designer thinks

"Put the contactors under the breaker, leave room for the ducts, will it fit
on a 600 x 800 plate?" The designer thinks in **devices on rails**, not in
coordinates. The tool should answer "does it fit" and "what is too close"
before any mouse work, and `place all` should give a usable first layout.

What goes wrong elsewhere: layouts drawn by hand drift from the schematic,
a device with no part has no size so overlaps go unseen, and clearances
(heat, wiring ducts, arcing) live in the designer's head.

## 2. Objects

| Thing | In the model | Stored | Derived |
| --- | --- | --- | --- |
| Panel | The enclosure `Structure` (`+K1B1G1`), one layout for all page groups (DESIGN §Pages) | its mounting plate size | contents, free area |
| Mounting plate | Property of the panel: width x height in mm, origin top left | size, margins | usable area |
| Rail / duct | Line object on the plate: DIN rail, wiring duct | position, length, kind | slots, fill |
| Placement | A device placed on the plate | x, y, rotation (0/90), optional rail | footprint, neighbours |
| Footprint | From the device's part: `width_mm`, `height_mm`, `depth_mm` | part data | rectangle on the plate |

A device without a part or without dimensions is **unsized**: it gets a
default 45 x 90 mm footprint marked "guessed size" and an Issues entry
("devices without a part"), never a silent zero. The drawer lists *unplaced*
devices first (DESIGN §3); unsized ones have no position of their own there.

## 3. Geometry rules

- Millimetres, plate origin top left, x right, y down. Rotation is 0 or 90
  degrees; the footprint swaps width and height at 90.
- A footprint is the part's width x height rectangle. Depth is not used on the
  plate, only against the enclosure depth (check).
- Devices on a rail sit at a rail offset along it; across the rail they are
  centred on it. Free placement (no rail) snaps to the Cabinet canvas grid (default 5 mm), a per-user setting like the
  schematic grid, so the two never disagree silently.

## 4. Queries (the engine's first job)

Pure functions of a snapshot, no mutation, so checks, the UI and scripts share
them:
1. `footprint(device)`: rectangle, plus whether the size was guessed.
2. `collisions(panel)`: pairs of placements whose footprints overlap, and
   placements that leave the usable plate area.
3. `clearance_violations(panel, rules)`: pairs closer than the required gap.
   Rules are a default gap (default 10 mm) plus per-kind overrides (for
   example drives 50 mm above and below). Gaps are measured edge to edge.
4. `fits(panel)`: plate area used, free area, and per rail the used length
   against its length (the "rails that would overflow" warning).

## 5. Commands (changes only through the command path)

`PlaceDevice`, `MovePlacement`, `RotatePlacement`, `RemovePlacement`,
`SetPlateSize`, `AddRail`. `place all` is a planner that proposes placements
(sort by letter code and page order, fill rails left to right, new rail when
full) and shows a preview; accepting it is one undo step.

## 5a. Cabinet canvas controls

Ctrl+2 opens the canvas, Ctrl+T switches to its Table face, `place all`
shows a preview and Enter accepts it as one undo step, and selection syncs
both ways with the drawer rows.

## 5b. Inside and door (R2)

The Cabinet canvas has a right panel, "Inside and door". Its form stores the
interior width, height and depth, the plate setback (back wall to plate
surface), whether there is a door, its hinge side (seen from the front), how
far it opens, how far things on its inside stand out, and the free room in
front of and beside the hinge side of the cabinet when known. Apply is one
undo step. The values are free-form properties of the enclosure `Structure`
(`novo_layout::model::keys`), so there is no `novo-core` change.

Checks (`novo_layout::enclosure::check`, planning values):

- Usable depth is interior depth minus plate setback minus the door's inside
  depth. A device whose part depth is larger is "too deep" (the door cannot
  close); one that leaves less than 10 mm is "tight". Devices whose part has
  no `depth_mm` are listed, not guessed.
- The plate must fit inside the interior.
- The open door, taken as wide as the interior, needs its width in front for
  an opening of 90 degrees or more (width times sin of the angle below 90),
  and width times -cos of the angle beside the hinge for more than 90. When
  the free room is known and smaller, the shortfall is a problem.

The top view in the panel draws the plate, each device as deep as its part,
the closed door, the arc of the open door and the free-room line. Not done:
the door is not checked against devices mounted on it (hole and cut-out work,
R5), hinge play and frame thickness are ignored, and the plate is assumed
centred across the interior.

## 6. Out of scope here

Wire ducts routing and lengths (`novo-routing`), 3D, door cut-outs, thermal
calculation. They read the same placements later.

## 7. Plan

1. This document.
2. `novo-layout` crate: plate, footprint, placement types and the four
   queries, unit tested; no model change.
3. Model storage of placements (WS-02 request) and commands.
4. `place all` planner.
5. Cabinet canvas drawing and drawer wiring (UI, thin).
