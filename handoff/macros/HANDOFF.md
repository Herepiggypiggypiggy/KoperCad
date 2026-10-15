# Macros: handoff

Repo Herepiggypiggypiggy/kopercad, branch `claude/project-thread-7fqkbd`. Design note merged (#396).
This thread owns placement; the Macro library thread (session_01TWr8q1k7WUQn2qomcUP6B6, #405,
handoff/macro-library) owns the .kcmacro file, the library folder and the browser window.

## State
- `novo_workflow::macro_place` (place_macro, preview): adopts the definition into the project
  (`Command::AdoptBlockDefinition`, #425), places it, settles wire numbers. Draft PR open for it.
- Parts and cabinet carry-over: not started (design step 4). App ghost + commands: not started.

## How to reach it (user-visible entry points)
Command ids agreed with UI integration (session_01MQYH2ZYReC8dTZsMHYf4pc): `macro.save` and
`macro.place` (this thread, rows registered by this thread), `macro.browser`, `macro.import`,
`macro.export` (Macro library thread). Menu: Tools, and Draw > Macros for place (Workspaces #438).
Shortcut proposal Ctrl+Shift+M for macro.browser (check keymap). None of these exist in the app yet;
the engine PRs say so under "How to reach it".

## Resume
Next: expose thin fns for macro.save (define_block + MacroLibrary save) and macro.place (ghost using
`preview`), register COMMANDS rows, then parts/cabinet offsets. Core gaps go to WS-02
(cse_01B1jpjzeDiXvWoBCsv1BWt9). Draft PRs, small, no build, no admin rights. After a push, message the
reviewer with the head SHA; once READY do not push.
