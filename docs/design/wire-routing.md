# Wire routing through ducts (WS-16)

How KoperCAD decides which way a wire runs inside the cabinet, how long it is,
and how full each duct gets. This extends `docs/design/cabinet-layout.md` (the
placements and rails it routes over) and `docs/DESIGN.md`; where they disagree,
DESIGN.md wins and this file is corrected. Engine code lives in `novo-routing`
and is pure geometry; the UI stays thin.

## 1. What the designer thinks

"How long is wire W17, which duct does it take, and will the duct overflow?"
Nobody wants to draw wires in the cabinet. The schematic already says what
connects to what; the layout says where devices sit. The tool should derive
the route, the cut length for the wire list and the duct fill, and show them
for checking.

What goes wrong elsewhere: lengths are typed by hand or guessed with a flat
allowance, duct fill is noticed at the workbench, and a moved device silently
leaves stale lengths behind.

## 2. Objects

| Thing | In the model | Stored | Derived |
| --- | --- | --- | --- |
| Duct | Line object on the plate (cabinet-layout §2): horizontal or vertical run with a kind | position, length, width, height | capacity, fill, nodes |
| Terminal anchor | A connection point of a placed device, in plate coordinates | offset from the device origin | position on the plate |
| Wire job | One connection that needs a physical wire: two anchors, a cross-section | from the netlist (wire list) | route, length |
| Route | The path chosen for a job | none (recomputed) | polyline, ducts used, length |

Routes are **derived, not stored**: they are a pure function of placements,
ducts and jobs, so a moved device changes them with no extra edit. Only
overrides the designer makes (see §6) are stored.

## 3. Geometry and units

- Millimetres, plate origin top left, x right, y down, as in the layout doc.
- Ducts are axis-aligned (horizontal or vertical). Diagonal ducts are not
  supported; they are rare and a polyline of straight ducts covers them.
- A duct has a cross-section `width x height` (mm). Its capacity is that area
  times a **fill limit** (default 0.5, a per-project setting, so wiring that
  is easy to pull stays easy). A wire occupies `pi/4 * d^2` where `d` is its
  outer diameter; the diameter comes from the cross-section table, with a
  guess when the part is unknown (marked guessed, as footprints are).

## 4. The routing graph

1. **Duct nodes.** Every duct end and every crossing or touching point of two
   ducts is a node. A duct between two nodes is an edge with its length.
2. **Anchor stubs.** A terminal does not sit on a duct. From each anchor a
   stub runs to the nearest point on a duct by Manhattan distance, but only
   in the direction the terminal faces (a terminal on top of a device exits
   upwards). A duct behind the terminal is still reachable: the wire leaves,
   turns and comes back, which costs twice the backwards distance as a
   **detour** (shown separately from the path). A stub that would run across
   another duct is not used while a clear one exists. The three nearest ducts
   are tried per end and the cheapest total wins; ties break on the lower duct
   id, so results are stable.
3. **Shortest path.** Dijkstra over the duct graph between the two stub entry
   points, edge cost = length plus a **fill penalty** that rises as the duct
   passes 80% of its capacity, so wires spread to a second duct before the
   first overflows. Equal costs break on lower node id.
4. **Length.** `stub + along ducts + stub + slack`. Slack is a fixed service
   loop per end (default 50 mm for terminals, configurable per kind), added
   once per end, never hidden inside the path.
5. **No duct path.** If both anchors reach the same duct network, route
   through it. Otherwise (no duct at all, or two disconnected networks) the
   job gets a **direct route**: an L-shaped Manhattan line between the anchors,
   flagged "free air", and a problem entry says so. It never fails silently
   and never returns length zero.

## 5. Order and fill

Jobs are routed in a fixed order (longest direct distance first, then job id)
so the result is deterministic and the big wires get the short paths. Fill is
accumulated as jobs are routed. After routing, `fill(duct)` is the used area
over capacity; a duct above 1.0 is an **overfull duct** problem naming the duct
and the jobs through it, a duct above the fill limit but not over capacity is
a warning.

## 6. Queries and commands

Pure queries (no mutation, shared by checks, reports and scripts):
1. `route_all(layout, ducts, jobs, rules)`: every route plus the duct fill.
2. `route(job)`: one job, used for a live preview while a device is dragged.
3. `duct_fill(result)`: per duct, used area, capacity, ratio, job ids.
4. `problems(result)`: free-air routes, overfull ducts, unsized wires.

Commands (changes only through the command path, one undo step each):
`AddDuct`, `MoveDuct`, `RemoveDuct`, `SetFillLimit`, and `PinRoute(job, ducts)`
to force a wire through chosen ducts. A pin is the only stored routing data.
`route all` shows a preview like `place all`; lengths flow to the wire list
(`novo-reports`) and the checks (`novo-checks`) through the query, never copies.

## 7. Out of scope here

3D routing, bend radius, wire bundles with twisted pairs, cable (as opposed to
wire) routing, door and cable-entry cut-outs, and tray or conduit outside the
cabinet. The cabinet canvas drawing of routes is UI work after the engine.

## 8. Plan

1. This document.
2. `novo-routing` crate: duct, anchor and job types, the routing graph,
   shortest path, length and slack, unit tested; no model change.
3. Duct fill, fill penalty, determinism tests, problems.
4. Ask WS-02 for duct and pin storage, then the commands.
5. Wire list lengths in `novo-reports`, checks in `novo-checks`.
6. Cabinet canvas drawing of ducts and routes (UI, thin).
