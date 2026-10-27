# Wire-it: the wiring pass, one wire at a time

Source: `audit/creative/round37.md` idea 1. The workshop wires by hand; hand wiring is most of the hours. Build order
puts all wires in one step (the closing one). Wire-it is the pass for the wires: one connection per step, in a
bench sequence, with both ends and the route lit in 3D, what to cut, strip and label, and how full each duct gets.

## Where the user finds it

- 3D tab > Layers > **Wire-it** (a toggle next to Build order), or View > 3D > Wire-it, or the command palette
  (`wire-it`). Commands: `view.3d_wire_it`, `view.3d_wire_it_next`, `view.3d_wire_it_previous`, `view.3d_wire_it_done`.
- The strip at the bottom of the 3D view: Previous, Next, Show all, **Print list (PDF)**, **Run, next wire**,
  Mark run (or Not run yet), Next not run, a slider to any wire.
- Reports > Export > **Export the Wire-it list (PDF)** (`export.wireit`): landscape A4, one dense row per wire.
- Wire-it and Build order never run together; opening one closes the other.

## The sequence (`novo_reports::wireit`, pure, no UI)

One step per connection of the wire production jobs (`wire_cut_jobs`), so the length, strip, ferrule, label and duct
list are the cut list's own. Order:

1. Pass: protective earth, power, 24 V supply, signals. Heavy stiff wires go in while the ducts are empty, the thin
   ones are laid last on top. Protective earth is part of the power pass and goes first, as the bonding.
2. Inside a pass the wires of one duct track stay together (a duct is filled in one visit). Tracks run shortest
   first (by their shortest wire); wires in a cable come after the routed ones, wires in no duct last.
3. Inside a track: short before long, then by wire number.

The class of a wire is a model of usual practice, not a reading of the standard: green-yellow is earth; a net with a
PE or PEN potential is earth; a line or neutral potential, a voltage above 60 V, or 2.5 mm2 and up is power; DC
potentials up to 60 V are the supply; the rest are signals.

The order never depends on what is ticked, so a half-wired cabinet resumes where it stopped. The cut-list order
(smallest size first, for the machine) and the bundle order (longest first, for laying) are unchanged: Wire-it is
the bench order, they are the machine and laying orders.

## Duct fill after each wire

For each duct a wire lies in: fill before and after, counted the way the routing counts it (wire area from the
cross-section, `novo_routing::model::wire_diameter_mm`, against the duct's allowed fill) and the number of wires in
it. The last wire of a duct ends at the fill the route screen shows (test
`the_duct_fill_of_the_last_wire_in_a_duct_is_the_route_screens_fill`). Over the limit shows red; a duct with no size
says so. The fill is the routing's model, not a measurement.

## Progress

A text property on the drawn wires of the connection, `wireit.done`: the keys of the connections run, `;` separated.
The key is the two pins (`symbol.pin>symbol.pin`, sorted), not the wires, because a T-junction is one drawn net and
several connections. It is in the project file, travels in a copy, and one tick is one undo step. A key whose
connection is gone (deleted or rewired after it was run) is counted and kept, and the strip says how many. Opening
Wire-it opens at the first wire not run. A tick keeps the plan (no new routing): the app moves the plan's revision
forward by its own edit.

## The picture

- The cabinet shows only the wires run so far and the wire to run, as the production job routed them (the same
  route, so the picture, the cut length and the fill agree).
- The wire to run: its path in warm white, floating just in front of the ducts it lies in (the overlay is depth
  tested, so a path inside a duct would be hidden), with a post down to each end; an orange diamond at the from end,
  a blue one at the to end; the ducts it lies in outlined in their fill colour.
- Wires run so far are drawn as thin green lines, floating the same way.
- The camera glides to the wire when the step changes.
- A step with no path on this plate (a feed, a wire to another cabinet, a core of a cable, an unplaced device) has no
  picture; the strip says why. It is on the sheet.

## The sheet (`novo_pdf::wireit`)

Landscape A4: box to tick (crossed when the wire is run in the project), number, wire and size, from and to, cut with
the end allowance, strip and ferrule, label at both ends, route and duct fill after the wire. Each page starts with
the pass heading; a heading is never alone at the foot of a page. Deterministic. Half-page cards (one wire per card)
are not made: 40 wires would be 20 sheets.

## Known limits

- The class is a model. A project without potentials or sizes puts everything in one pass.
- A T-junction's legs share drawn wires: each leg is its own step and its own tick, but the picture shows the leg's
  own route, not the drawn net.
- Wire-it reads the project it is shown, so with an Options variant selected the ticks are written to the working
  project and the plan is read from the variant.
- Not built: continuity test sheet (round 37 idea 2), kitting (idea 3).
