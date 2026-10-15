# Wire numbering: handoff

State (2026-10-15): engine, macro integration and UI written on branch claude/project-thread-lhuvv3
(kopercad). New crate `crates/novo-numbering` is not in `[workspace.dependencies]`; dependents use a
path dependency (novo-workflow, novo-app). The coordinator should register it in its own small PR and
the path lines can then become `.workspace = true`.

Files: crates/novo-numbering/src/{scheme,group,plan,check,manual}.rs; tests/numbering.rs (14 tests);
novo-workflow macro_place.rs (`WireNumbers::Auto|Keep|Clear`, report.numbered_wires);
novo-app src/app/numbering.rs + numbering_tests.rs; commands wires.* in novo-workflow commands.rs
(with Dutch words in language.rs).

Resume: `cargo test -p novo-numbering -p novo-workflow -p novo-app numbering`.

Decisions: numbers are per net, stored on every wire of the net. Fill never touches a unique existing
number. Locked = wire property `wirenum.locked`. Scheme in project properties `wirenum.*`; no novo-core
API change was needed. Plans are refused when the project revision moved.

Talk to: macros thread (macro_place change), wire list / labels / cut list threads (they read
Wire.number, no change needed), checks thread (wrap `check`).
