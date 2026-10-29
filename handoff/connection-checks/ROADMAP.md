# Connection-point checks: roadmap

Scope: use what a part states about each connection point (wire range, strip length, torque, conductors per point, ferrule rule) in Check > Issues, the wire and terminal reports and the Terminals drawer. Source item 4 of FINAL-PRIORITIES (private).

Done
- Data model: `PartTerminal` gains `strip_mm`, `torque_nm`, `max_wires`, `ferrule`; `novo_parts::pins` writes them to project part properties as `pin.<marking>.*` and reads them back with `connection_data`.
- Checks in `novo-checks/src/conn_points.rs`: `WireRange`, `PointOccupied`, `FerruleUse`.
- Wire list and terminal diagram get a Torque column; the cut list takes the point's strip length before the part-level one.
- Terminals drawer gets an Accepts column.

Next
- Parts XML importer fills the new `PartTerminal` fields and sets `sources["connection_data"] = MakerDatasheet` (importer thread owns this).
- Bench list and labels: show strip length per end from the point (already used by the cut list).
- Multi-level terminals: marking per level once the importer's naming is known.

Open questions
- Whether a point with no data should get an Info "not known" finding; left out so existing projects stay quiet.
