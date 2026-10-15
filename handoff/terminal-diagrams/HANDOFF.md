# Terminal diagrams: handoff

Split (Supervisor, 2026-10-15): this thread owns the graphical strip diagram (bridges/jumpers, end plates) and its PDF/CSV/drawer; "Cable and terminal strip plans" (session_01RZjDVCS7GLdP7YT6jnyYoF) owns cable schedule, tabular strip plan ; this thread owns the plug list (`plugs`, crates/novo-reports/src/plugs.rs, from d56063e of the cable thread).
Data: novo-terminals `screen` (ScreenRow: wires_internal, wires_external, bridged_with). Report: novo-reports diagrams.rs `TerminalDiagram` columns incl. Wire in, Bridged to, Wire out, Location.
Next: strip_diagram.rs module with a drawn strip (blocks, jumper bars, end plates), then PDF.
Check: cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
