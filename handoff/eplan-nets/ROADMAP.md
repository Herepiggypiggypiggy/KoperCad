# EPLAN-NETS roadmap

Scope: nets of imported EPLAN data rebuilt without EPLAN, and EPLAN-style checks in novo-checks. Design: `docs/design/eplan-nets.md`.

Done
- NETS-2: the 13 remaining portable rules (house_rules.rs).
- `novo_interop::eplan::nets`: ConnGraph, rebuild_nets, compare, spanning_links, unit tests and `tests/eplan_nets.rs`.
- Seven checks in novo-checks (`connection_rules.rs`, `tests/eplan_style.rs`).

Next
- Adapter from EPLAN-PXF-READ's reader to `ConnGraph` (theirs; the interface was sent to cse_01SaP8ogQQJ4nBhYv3qCTcBm).
- Call `compare` after the import creates wires and show `NetComparison` in the import preview.
- C08, C12, C16, C18 and the opt-in real-sample tests.

Open
- Real `.epj` sample to confirm lines ending inside a pin, non-orthogonal lines and the node leg table.
- Private sample at /mnt/project-files/interop/private/ (README there): real data validation of node legs and interruption pairing waits for the PXF reader's pxf::conn; opt-in tests via KOPERCAD_PRIVATE_SAMPLES only, no sample content in the repo.
