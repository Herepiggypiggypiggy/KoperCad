# Macro library: handoff

Branch claude/project-thread-mrizk0 restarted from main after #405 merged; no open PR.
Code: crates/novo-parts/src/macros.rs, tests in crates/novo-parts/tests/macros.rs. No novo-core change.
Format: `<name>.kcmacro` JSON {format:"kopercad-macro", version, name, description, author, tags, revision, definition: BlockDefinition}; packs `.kcmacropack` hold several with folders.
User folder: %APPDATA%\KoperCAD\Macros (user_macros_dir); BlockSource.library = "<label>/<relative path>", version = file revision.
Resume: `cargo fmt --all && cargo clippy -p novo-parts --all-targets -- -D warnings && cargo test -p novo-parts`.
People: Macros thread cse_01LeNjYNK8vqQz2hGZ1qk49q; integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S; main reviewer cse_016C9nTT8cN7voW8t1NQUG4W.

## How to reach it (user-visible entry points; engine is merged, UI is not yet)
Engine API (novo-parts): MacroLibrary::user(), list/search/load, save/replace/rename/move_to/trash/restore, import/export/export_pack, install_starters, definition_for_project.
Planned entries (Tadjiro's rule: an engine without a UI entry is not done):
1. Macro browser window (tree of folders, search field, description, Rename/Move/Delete(trash)/Restore) - command `macros.browse`, Ctrl+K words "macro", "macros", "bibliotheek". Owner: this thread builds the window; UI integration thread places the menu/command entry.
2. Import macro file or pack... / Export selected... buttons in the browser (`.kcmacro`, `.kcmacropack`), also reachable from Ctrl+K "import macro".
3. Starter set: installed on first open of the browser (folder Starter) and by command "Install starter macros".
4. Place button in the browser: waits on Command::AdoptBlockDefinition (WS-02) and the Macros thread's placement helper.
