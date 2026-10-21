# Auto-wiring by pin kind: handoff

Branch `claude/project-thread-yflewj` (kopercad). Code: `crates/novo-editor/src/pinkind.rs`, `edit.rs` (`facing_checked`, `Facing`, `HeldWire`), `tool.rs` (`Editor::facing`, `explain_facing`, `propose`).

Rules (heuristics, not exact): Power vs Control and Earth vs anything but terminals are refused (never drawn, never proposed). Odd/odd or even/even designations, and crossed poles between power devices, are "ask": proposed in Ask mode, not drawn in Draw mode. Unknown symbols are never refused. Manual wires are not judged.

Resume: `cargo test -p novo-editor --test pin_kind_wiring`.
Trap: `Editor::leave_tool` clears the notice, tests must not call it before reading the notice.
