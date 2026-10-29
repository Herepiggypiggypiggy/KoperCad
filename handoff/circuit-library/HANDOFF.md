# Circuit library: handoff

Branch `claude/project-thread-088iiz`. Code: `crates/novo-parts/src/macros/circuits/`
(spec.rs, build.rs, mounting.rs), data `crates/novo-parts/circuits/`, UI
`crates/novo-app/src/app/macro_mounting.rs`, tests
`crates/novo-parts/tests/circuit_library.rs` and
`crates/novo-tests/tests/circuit_library_clean.rs`.

Resume: `cargo test -p novo-parts --test circuit_library`, then
`cargo test -p novo-tests --test circuit_library_clean` (slow, builds the
workspace). Preview a file: `cargo run -p novo-parts --example circuit_preview`.

Traps: new files must be listed in SOURCES; starters stay in folder `Starter`
and their counts are asserted elsewhere; wires name library pin ids (13/14),
not renumbered ones; a chain of bus taps needs separate junctions or the
checks report WireOverlap.
