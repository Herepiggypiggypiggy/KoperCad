# Macro library: handoff

Branch claude/project-thread-mrizk0 restarted from main after #405 merged; no open PR.
Code: crates/novo-parts/src/macros.rs, tests in crates/novo-parts/tests/macros.rs. No novo-core change.
Format: `<name>.kcmacro` JSON {format:"kopercad-macro", version, name, description, author, tags, revision, definition: BlockDefinition}; packs `.kcmacropack` hold several with folders.
User folder: %APPDATA%\KoperCAD\Macros (user_macros_dir); BlockSource.library = "<label>/<relative path>", version = file revision.
Resume: `cargo fmt --all && cargo clippy -p novo-parts --all-targets -- -D warnings && cargo test -p novo-parts`.
People: Macros thread cse_01LeNjYNK8vqQz2hGZ1qk49q; integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S; main reviewer cse_016C9nTT8cN7voW8t1NQUG4W.

## How to reach it (user-visible entry points; engine is merged, UI is not yet)
Engine API (novo-parts): MacroLibrary::user(), list/search/load, save/replace/rename/move_to/trash/restore, import/export/export_pack, install_starters, definition_for_project.
Entries (browser built in the second PR):
1. Macro browser window (tree of folders, search field, description, Rename/Move/Delete(trash)/Restore) - command `macros.browse`, Ctrl+K words "macro", "macros", "bibliotheek". Owner: this thread builds the window; UI integration thread places the menu/command entry.
2. Import macro file or pack... / Export selected... buttons in the browser (`.kcmacro`, `.kcmacropack`), also reachable from Ctrl+K "import macro".
3. Starter set: installed on first open of the browser (folder Starter) and by command "Install starter macros".
4. Place button in the browser: waits on Command::AdoptBlockDefinition (WS-02) and the Macros thread's placement helper.

## FU-MACRO follow-ups (2026-10-16, branch claude/project-thread-piifil)
Done in one PR (see PROGRESS): parts field is typed and bumps the file to format version 2 only when parts are carried (MACRO_BASE_VERSION 1, MACRO_FORMAT_VERSION 2 = newest readable); thumbnails (novo-parts macros/extras.rs `thumbnail`, painted by novo-app macro_thumb.rs); company folder (Settings.company_macros, Preferences > Macros; browser switch My macros / Company; "Copy to my macros" because Place reads the user library only); update offers (`macro_updates`, shown in the Macros window when the open project holds older revisions; the action renames the old project definition to "<name> (rev N)" so the new one can be placed, because updating placed circuits in place needs a novo-core command, to ask WS-02); `kopercad-cli macros list|export|import|install-starters [--library DIR]`.
Open: in-place update of placed instances (WS-02); Place from the company folder without copying; thumbnail text labels.
