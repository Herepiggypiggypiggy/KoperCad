# EMC rule engine: roadmap

Scope: the `novo-emc` crate (pure engine) and its entries in Check, the Issues list and the standards report. Plan: `docs/EMC.md`; lead and visualisation lane: `handoff/emc/`.

## Done (E1 and part of E2)
- Registration of `novo-emc` (#1381).
- Wire classes (motor, power, control, analog, fieldbus, safety, earth): derived from the drawing, overridable per wire or cable (`emc.class`), each with its reason and origin.
- Rules: EMC-01 (classes in one duct or cable), EMC-02 (distance on parallel stretches, edge to edge, shield and twisting credit, divider), EMC-04 (motor cable shielded and terminated), EMC-05 (pigtail impedance, estimate, band to 30 MHz), EMC-07 (drive filter present and close), EMC-09 (loop area from the two paths), EMC-11 (painted plate, case not bonded), EMC-14 (signal leaving the cabinet unshielded), EMC-17 (sensitive device near a drive).
- Scores per run, device, plate and net: sum of listed reasons, capped at 100.
- Fixes: DeclareClass, AddShield, Terminate360, AddFilter apply as one undo step; the rest are by hand and say so.
- `Picture` (runs, ribbons, loop polygons, zones, hotspots, assumptions, not checked) for the visualisation lane.
- Checks: `ProblemKind::Emc` (opt-in `emc.check`), waiver key `check.emc.<rule>`, EMC family in the standards document, Check > EMC check window.

## Next, in order
1. EMC-03 crossing angles (only matters once diagonal routes exist), EMC-06 shield through terminals (needs a shield terminal kind), EMC-08 motor cable length against part data (needs part EMC data from the Parts lane), EMC-10 return through the plate, EMC-12 door bonding, EMC-13 suppression of inductive loads, EMC-15 24 V supply and return together, EMC-16 surge at the entry, EMC-18 ferrite advice.
2. Filtered against unfiltered mains classes (needs the filter topology).
3. A move-run fix that re-routes with the duct excluded; needs a routing hook (a per-wire duct preference) agreed with the routing owner.
4. Validation document numbers (Neumann mutual inductance against the image-method formula) for `docs/emc-validation.md` (the lead owns the file).
5. Level 3 field model (third lane, after E2).

## Open questions
- Typed `novo-core` fields instead of properties: not before the visualisation lane has used the properties for a while.
- Default distances are planning values from common practice; replace by a maker's value where a part states one.
