# rel-contactor handoff

Branch claude/project-thread-rofgkd, one PR from main. Changes: symbol keywords (iec and nfpa contactors/contacts), `crates/novo-workflow/src/insert.rs` search (multi-word, favourite boost), tests in insert.rs and `crates/novo-editor/tests/editing.rs`.
Finding: the wiring bug was not reproducible with the right symbol; it came from placing the coil. Resume: `cargo test -p novo-workflow -p novo-editor`.
