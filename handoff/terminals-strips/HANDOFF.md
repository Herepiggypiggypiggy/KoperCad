# Terminals and strips: handoff

Branch claude/project-thread-bqa3bb (repo kopercad). Engines: novo-editor row.rs (pole_row), edit.rs (adjacent_strip, strip_for), novo-terminals move_strip.rs (move_to_strip), strip.rs (connect_terminals), style.rs (connected symbol), geometry.rs (one_ended). Symbols: terminal_end, terminal_pe_end, terminal_n_end, terminal_bridged, terminal_end_bridged. App: terminals.rs terminal_command; commands terminal.* in novo-workflow.
Decisions: Editor.strip no longer forces the strip; adjacency (20 mm, in line) decides, else next free -X. Alias `x` is one-ended, `term` stays two-ended.
Check: cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
