# EPLAN-NETS roadmap

Scope: nets of imported EPLAN data rebuilt without EPLAN, and EPLAN-style checks in novo-checks. Design: `docs/design/eplan-nets.md`.

Done
- `novo_interop::eplan::nets`: ConnGraph, rebuild_nets, compare, spanning_links, unit tests and `tests/eplan_nets.rs`.
- Seven checks in novo-checks (`connection_rules.rs`, `tests/eplan_style.rs`).

Next
- Adapter from EPLAN-PXF-READ's reader to `ConnGraph` (theirs; the interface was sent to cse_01SaP8ogQQJ4nBhYv3qCTcBm).
- Call `compare` after the import creates wires and show `NetComparison` in the import preview.
- More checks from the crane repo's `PROJECTCONTROLE.md` section 7 once its list is available.

Open
- Real `.epj` sample to confirm lines ending inside a pin, non-orthogonal lines and the node leg table.
