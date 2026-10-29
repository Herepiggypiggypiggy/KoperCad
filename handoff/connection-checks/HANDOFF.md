# Connection-point checks: handoff

Branch `claude/project-thread-yoh0a4` (kopercad). Data keys per point, on the project copy of a part (`novo_core::Part::properties`):
`pin.<marking>.wire_min_mm2`, `.wire_max_mm2`, `.strip_mm`, `.torque_min_nm`, `.torque_max_nm`, `.max_wires`, `.ferrule` (`required` | `optional` | `forbidden`), plus `pin_data_source` (`maker` | `typed` | `estimated` | `unknown`). Written by `CatalogPart::to_model` from `PartTerminal`; the source comes from `CatalogPart::sources["connection_data"]`.

Rules: nothing is judged where the part states nothing; findings name the source. Wire ferrule is the wire property `ferrule` (`none` turns it off) and `end_processing`, as novo-reports reads them.

Where to find: Check > Issues (kinds "Wire size at terminal", "Terminal point full", "Ferrule at terminal"), Terminals drawer (Accepts column), Reports > Wire list and Terminal diagrams (Torque column).

Tests: `cargo test -p novo-checks --test conn_points`, `-p novo-parts --lib pins`.
