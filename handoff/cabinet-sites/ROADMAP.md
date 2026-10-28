# Cabinet sites and sizing (laptop test report findings 9, 10, 18, 24, 32, 34)

Scope: what belongs on a mounting plate, and the cabinet bits the cooling tower report found wrong.

Done (PR 1, finding 9 and 10):
- `novo-layout::site`: `mounting_site` property (plate, door, field); motors (M) and actuators (Y) are in the field by letter code.
- Field devices are not Place all candidates, not unplaced, not counted in the enclosure sizing.
- Push buttons, lamps and meters (H, S, P) go to the door by default surface rules when the enclosure has a door; a device can say `plate` or `door` itself.
- Drives and soft starters (T with `power_kw`) get a power-class footprint, marked guessed.
- Check `FieldDeviceInCabinet` (warning) for a field device that sits on a plate.
- Cabinet inspector row "Mounted" (cabinet, door, field); Size enclosure lists what it did not count.

Next:
- PR 2: plate resize carries rails and ducts (18), Table toggle feedback without a plate (34).
- PR 3: contents row count and duplicate page names (32), default cabinet and page names (24).
- Heat: a field device in the cabinet still adds its loss to the heat map; the warning is the only guard so far.
- Wiring openings have no editor (structure property `opening.<name>`), so wires to door devices stay estimates.
