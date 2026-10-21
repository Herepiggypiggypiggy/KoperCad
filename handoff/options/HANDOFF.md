# Machine options: handoff

Branch `claude/project-thread-e6bexd` (kopercad). No model change: options live in `properties` (`kc.options`,
`kc.when`), see docs/design/variants.md "Options on the drawing".

Resume: `cargo test -p novo-app --lib machine`, `cargo test -p novo-workflow --test options`,
`scripts/qa.sh --only options`. Rewrite the fixture with
`cargo test -p novo-app --lib write_machine_fixture -- --ignored`.

Traps: the machine copy has its own project revisions, so caches keyed on `project.revision()` also mix in
`machine_generation()`. A tie replaces an object's condition. Cabinet/Commissioning intentionally use the working
project. Ask the Data model thread nothing: no core change.
