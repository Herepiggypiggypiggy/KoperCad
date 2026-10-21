# Auto-wiring by pin kind: roadmap

Scope: automatic wires between facing pins (Draw and Ask modes) judge what the pins are, not only where they sit. Audit defect C-11.

Done
- `novo-editor/src/pinkind.rs`: pin kind from the library symbol (power, control, earth, neutral, terminal, single-pole) and a verdict per pair: wire, ask, refuse.
- `edit::facing_checked`: facing pairs split into wires and held pairs; `facing_wires` stays geometry only.
- Editor status line says what was wired, what was held and why; Ask mode also proposes the unusual pairs.
- Tests in `crates/novo-editor/tests/pin_kind_wiring.rs` (motor starter, reversing starter, coil under motor, crossed poles).

Next
- A real pin-kind field in the symbol format would replace the heuristics (needs WS-05 and WS-02).
- Same judge for the wire tool's drag-to-pin hints and for xref/terminal auto joins.
- Check 4-pole and PLC symbols against the heuristics.

Open questions: should a one-pole breaker feeding a coil be drawn automatically? Today yes (control feed).
