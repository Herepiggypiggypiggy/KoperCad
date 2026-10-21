# Cabinet entry point and field wire lengths

Why: wires that do not stay on the mounting plate are the last estimates in the
wire list and the Ready meter. In the shipped example all ten are of two kinds:
the motor wires that leave for a device in another location (a cable run) and
the supply feeds that come in from outside. The routing engine cannot route
them, because one end is not on a plate (`docs/design/wire-routing.md` §6a:
"unrouted"). Placing ducts does not change that.

Owner: Autopilot thread. The WS-02 thread agreed to the property keys and the validation rules in conversation; no change to `novo-core`.

## What is stored

| Thing | Where | Shape |
| --- | --- | --- |
| Entry point | Enclosure structure `properties` | `entry.x_mm`, `entry.y_mm`: Number, plate coordinates, origin top left |
| Wire outside the entry | Enclosure structure `properties` | `entry.outside_mm`: Number, mm of wire from the entry to where it ends, loops included; for wires without a cable |
| Cable length | `Cable.length_m` (exists) | metres, **the whole cable as bought**, between its two ends |

Rules for the entry point (WS-02): both keys set or neither; finite; inside
the plate; written with `Tx::set_property`. A plate with no entry point has
none. A stored point that no longer fits a smaller plate reads as none and the
panel says so.

## How a length is found

Wires that stay on one plate are routed over its ducts as before. For a wire
with an end outside the plate:

1. **On a cable with a stated length**: the cable length as stated, no loop
   added (the stripped ends are part of the cable as bought). Basis `cable length`, not an
   estimate. The cores run the whole cable, so no entry point is needed; if the
   wire is also routed to the entry (see 2) its route only fills the duct. If
   one connection has wires on several cables, the longest cable wins.
2. **Without a cable**, one end on a placed device and the other outside (a
   supply lead from a potential feed, or a device whose `+location` differs
   from the enclosure's; a tag without a location is *unknown*, not outside):
   routed over the ducts to the entry point. The route holds a service loop at
   the placed terminal and one at the entry. With `entry.outside_mm` stated the
   length is route + outside, basis `to entry + stated outside`, not an
   estimate.
3. Otherwise the whole-wire page estimate, labelled `estimate`, as before.

## What is exact and what is not

Exact (to the model): the inside part, when devices and ducts are placed.
Stated by the designer: the cable length and the wire outside the entry. Never
invented: an unknown outside part is not replaced by a number; the wire keeps
the estimate label.

## Autopilot

A plate with supply leads that are still estimates and no entry point gets one
row: "entry point at the bottom middle of the plate" (basis Assumed, so
unticked), because where the gland really is, the designer knows. The cable
rows (accept them, then state each cable's length) and the entry plus the wire
outside it are what turn the remaining estimates exact; the window says so.

## Plan (small PRs)

1. This note; the entry point accessors with the WS-02 validation, and a field
   in the Cabinet routes panel (done, #1160).
2. Routing to the entry point in `novo-routing::model` (`Routed::entry_legs`)
   and the length rules in `novo-reports::lengths` (done, #1163; the cable
   meaning was corrected in step 3 after review).
3. Cable length as bought, wire outside the entry, empty location is unknown,
   Autopilot entry row (this step).
4. The entry and the runs in the 3D view: the entry is marked on the plate, a
   wire drawn to it shows where it leaves, and the Run card says how the
   length was found (done,
   `handoff/cable-3d/`). The outside part stays a stated length and is not
   drawn.
