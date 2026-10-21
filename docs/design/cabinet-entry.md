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
| Cable run | `Cable.length_m` (exists) | metres, the length outside the cabinet between the two enclosures' entries |
| Feed allowance | project wire rule `wire.feed_outside_mm` | mm of wire outside the entry; unset = unknown |

Rules for the entry point (WS-02): both keys set or neither; finite; inside the
plate; written with `Tx::set_property`. A plate with no entry point has none.

## How a length is found

For a wire with exactly one end on a placed device of a plate (the "inside
end") and the other end elsewhere:

1. Inside part: the routed length from the inside end to the entry point, using
   the same graph, stubs, slack and bend rules as any other wire (the entry is
   an anchor with no facing, at the plate surface).
2. Outside part:
   - the wire lies on a cable with `length_m`: that length, plus the service
     loop at the far end;
   - a feed, `wire.feed_outside_mm` set: that value;
   - otherwise unknown.
3. Length = inside + outside. Basis text:
   - `to entry + cable run` (both parts known): not an estimate;
   - `to entry + stated outside` (feed allowance set): not an estimate;
   - `to entry, outside unknown` (inside part routed, outside from the page
     estimate): still an estimate, and says which half is exact.

A wire with both ends outside any plate, or with no entry point on its plate,
stays `estimate` exactly as today. Nothing changes for wires already routed.

## What is exact and what is not

Exact (to the model): the inside part, when devices and ducts are placed. Stated
by the designer: the cable run and the feed allowance. Never invented: an
unknown outside part is not replaced by a made-up number; it keeps the estimate
label.

## Autopilot

A plate with wires that need an entry and has none gets one row: "Entry point
at the bottom middle of the plate" (basis Assumed, so unticked), because where
the gland really is, the designer knows. Cable runs and feed allowances are
listed under "left to you" with the count, as now.

## Plan (small PRs)

1. This note; the entry point accessors with the WS-02 validation, and a field
   in the Cabinet side panel to set it (where the user finds it).
2. (done) Routing to the entry point in `novo-routing::model` (`Routed::entry_legs`) and the new basis `to entry + cable run` in `novo-reports::lengths`, `is_estimate`; tests on the motor starter. A wire to the outside without a stated cable run stays `estimate`. The feed allowance is step 3.
3. Autopilot: the entry-point row and the refined "left to you" text.
