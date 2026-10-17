# ui-macro-authoring handoff

Branch `ui-macro-authoring`, one PR from main. Files: `crates/novo-app/src/app/macro_editor.rs` (window, tests in `macro_editor_tests.rs`), `crates/novo-parts/src/macros/params.rs` (engine), browser button and `macro_export_pack` in `macro_browser.rs`.

Decisions
- The editor writes the library file (revision grows), not a project definition: changing a project's `BlockDefinition` needs a novo-core command, which only WS-02 adds.
- Company macros are read-only in the editor.
- The parameter type is the type of the default value (text, number, whole number, yes/no).

Resume: `cargo test -p novo-parts --test macro_params`, `cargo test -p novo-app macro_editor`, `cargo test -p novo-workflow`.
