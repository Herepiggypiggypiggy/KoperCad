# Macros: handoff

Repo Herepiggypiggypiggy/kopercad, branch `claude/project-thread-7fqkbd`. Design note merged (#396).
This thread owns placement; the Macro library thread (session_01TWr8q1k7WUQn2qomcUP6B6, #405,
handoff/macro-library) owns the .kcmacro file, the library folder and the browser window.

## State
- Merged: design (#396), `novo_workflow::macro_place` place_macro + preview (#473).
- Draft PR: app entry points. `macro.save` (Save as macro window over `macro_place::capture`) and
  `macro.place` (Place macro window: search, parameters, preview line, Place; lands right of the
  drawing and selects the result). Code in `novo-app/src/app/macro_actions.rs`.
- Parts carry-over: draft PR (parts saved in the macro file's `parts` field, assigned on placement).
- Not started: cabinet offsets, a pointer-following ghost (the window
  previews instead), terminals inside macros (WS-02), update from library.
- Browser (`macro.browser`, import/export) is the Macro library thread's #455; its disabled Place
  button can call `open_macro_place(Some(name))` once both are merged.

## How to reach it (user-visible entry points)
Command ids agreed with UI integration (session_01MQYH2ZYReC8dTZsMHYf4pc): `macro.save` and
`macro.place` (this thread, rows registered by this thread), `macro.browser`, `macro.import`,
`macro.export` (Macro library thread). Menu: Tools, and Draw > Macros for place (Workspaces #438).
Shortcut proposal Ctrl+Shift+M for macro.browser (check keymap). macro.save and macro.place exist
in the app (Ctrl+K: "save macro", "place macro"); menu placement follows Workspaces #438.

## Resume
Next: expose thin fns for macro.save (define_block + MacroLibrary save) and macro.place (ghost using
`preview`), register COMMANDS rows, then parts/cabinet offsets. Core gaps go to WS-02
(cse_01B1jpjzeDiXvWoBCsv1BWt9). Draft PRs, small, no build, no admin rights. After a push, message the
reviewer with the head SHA; once READY do not push.
