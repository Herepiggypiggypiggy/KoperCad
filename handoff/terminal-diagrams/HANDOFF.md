# Terminal diagrams and plug lists: handoff

Data: novo-terminals `screen` (ScreenRow: wires_internal, wires_external, bridged_with, location via report) and `plug.rs` (`connectors`, `plug_list`, Contact). Reports: novo-reports diagrams.rs `TerminalDiagram` (columns incl. Wire in, Bridged to, Wire out, Location) and `PlugList` (id `plugs`, registered in standard_reports).
Contacts are grouped by pin name across all drawings of a connector device (DeviceKind::Connector). No mating-half logic yet.
Check: cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
