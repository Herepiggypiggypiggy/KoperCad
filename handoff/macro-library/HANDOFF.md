# Macro library: handoff

Branch claude/project-thread-mrizk0, draft PR "Macros: library folder, search and sharing files" (see PROGRESS for SHA).
Code: crates/novo-parts/src/macros.rs, tests in crates/novo-parts/tests/macros.rs. No novo-core change.
Format: `<name>.kcmacro` JSON {format:"kopercad-macro", version, name, description, author, tags, revision, definition: BlockDefinition}; packs `.kcmacropack` hold several with folders.
User folder: %APPDATA%\KoperCAD\Macros (user_macros_dir); BlockSource.library = "<label>/<relative path>", version = file revision.
Resume: `cargo fmt --all && cargo clippy -p novo-parts --all-targets -- -D warnings && cargo test -p novo-parts`.
People: Macros thread cse_01LeNjYNK8vqQz2hGZ1qk49q; integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S; main reviewer cse_016C9nTT8cN7voW8t1NQUG4W.
