# Circuit ghost handoff
Branch claude/project-thread-5max7e. Code: crates/novo-app/src/app/macro_actions.rs (begin_macro_ghost, after_macro_ghost, macro_ghost_strip, ghost_outline), hooked in workbench.rs (Target::Macro), clipboard.rs (after_paste_click) and app.rs.
The ghost reuses the editor's Paste tool; the click is read in after_paste_click and placed via place_definition(at).
The old form is unchanged: menu/command `macro.place` and the library browser's Place still open it; Options on the ghost strip opens it.
Trap: scenarios that picked a circuit by name then clicked Place Button now click Options first.
