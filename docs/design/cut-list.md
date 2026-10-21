# Wire list cut lengths and the cut list

How the wire list gets a length per wire and how the cut list turns those into
what the workshop cuts. Code: `crates/novo-reports` (`lengths.rs`, `wires.rs`,
`cutlist.rs`). Routing itself is `docs/design/wire-routing.md`.

## Wire list: Route length mm and Length basis

Two columns at the end of the wire list. Each connection (pin to pin) is
measured one of three ways, shown in *Length basis*:

| Basis | When | Length |
| --- | --- | --- |
| `routed` | both devices are placed on the same mounting plate and a duct path exists | `novo_routing::model::route_connections`: stub + along ducts + stub + 50 mm service loop per end |
| `free air` | same plate, but no duct reaches both | Manhattan line between the anchors + the service loops; the routing plan reports it as a problem |
| `estimate` | a device is not placed, or the ends are on different plates | see below |

Lengths are rounded up to a whole millimetre and are never zero.

**Estimate.** The Manhattan distance between the two pins on the schematic
page in millimetres, at least 100 mm; 500 mm when the pins are on different
pages; plus a 50 mm service loop at each end. The schematic is not drawn to
cabinet scale, so this is a planning figure only: it is always labelled
`estimate`, and it is replaced by the routed length as soon as both devices
are placed. The constants are `ESTIMATE_MIN_MM` and `ESTIMATE_ACROSS_PAGES_MM`
in `lengths.rs`.

The wire's outer diameter (for duct fill) is derived from its cross-section:
the copper circle plus 0.7 mm insulation each side.

## Cut list (`cutlist`)

A report built on the wire list's rows, so it never disagrees with it. It is
listed in the standard reports (CSV, Excel, PDF through a generated page) but
is not added to a project's pages automatically; add the page by hand.

- Summary table first: wires and total length in metres per group.
- One table per cross-section and colour (ascending size, size unknown last).
- Rows longest first, then by wire number.
- *Cut mm* = wire list length + an end allowance at both ends: 10 mm for a
  ferrule, 20 mm for a cable lug from 25 mm² up (`CutList` fields; defaults
  until the project rules hold them).
