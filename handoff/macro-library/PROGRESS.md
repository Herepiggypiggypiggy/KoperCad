2026-10-15 18:30 UTC engine written, 9 tests; format proposal sent to Macros thread
2026-10-15 18:40 UTC Macros thread agreed shape; added unknown-field preservation and .trash (trash/restore); 11 tests
2026-10-15 18:50 UTC reviewer READY on 17b877a; skip symlinks in walk/folders
2026-10-15 19:10 UTC starter macros (DOL, star-delta, E-stop chain, 24 V supply) built via define_block, install_starters(), pin tables checked against the IEC pack; 15 tests
2026-10-15 18:38 UTC #405 merged (library, trash, symlink safety, starter set). Next work from main: browser window and Place button wait for Command::AdoptBlockDefinition (WS-02, requested by Macros thread); thumbnails; CLI.
2026-10-15 19:30 UTC macro browser window (macro.browser, macro.import) with tests; Place button waits on Macros thread
2026-10-15 22:25 UTC #455 merged with main; Place button now opens macro.place (open_macro_place) since #489 is in
- 2026-10-16: merged main into #455 (conflicts in app.rs, workbench.rs; kept both sides), fmt/clippy/tests green; sent SHA to reviewer C.
2026-10-16 08:00 UTC FU-MACRO PR (branch claude/project-thread-piifil): .kcmacro `parts` field typed (MacroFile::parts/set_parts, format v2 only when parts are carried, v1 files load), thumbnail data (novo_parts::thumbnail) painted in the browser, company folder (Preferences > Macros) with My macros / Company switch, update offers (macro_updates) with "Set old copy aside", kopercad-cli macros list/export/import/install-starters. Tests green.
