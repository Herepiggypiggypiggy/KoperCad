# Maker parts XML importer

Goal: drop in a maker's parts XML (part, connection points, construction, accessories) and get a
catalog part with terminals, wire ranges, torque and drilling. Doc: `docs/PARTS-MAKER-XML.md`.

Done: `novo-parts::dataportal` reader, schema fields (`DrillHole`, `Accessory`, terminal
torque/strip/AWG/max wires/maker position), inbox and EPLAN-export dialog routing, merge that
keeps price and ETIM, tests on invented files.

Next, in order:
1. A real maker file (names only is enough: Copy format report) to adjust the attribute matching.
2. Confirm the position frame, then fill `PartTerminal::position_mm` and feed cabinet layout.
3. Drill plan output from `CatalogPart::drilling` (Cabinet > Output > Drill plan, item 3).
4. Wire-range and torque checks from the terminals (item 4).
5. Accessories offered on place (end plates, jumpers).
6. Contact counts and coil frequency are not read yet; rated voltage is stored as rated_voltage_v. Match the recipe field names of PR 1702 once it has merged.
