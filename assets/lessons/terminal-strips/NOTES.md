# terminal-strips: notes for the reviewer

Teaches: accept terminal proposals (Enter, Tab, accept all), read the Terminals drawer, bridge two terminals
(Connect terminals), switch PE by hand and give it back, move the PE terminal to strip -X2 (Up). 12 steps, about 9 minutes
(replay about 90 seconds, the rest is reading). `status` unchanged (`draft`); `version` is 2 because the wording changed.
Project: crates/novo-tests/tests/lessons/terminals.rs `terminal_strips_start` / `_finished` (pump, converter, two valves;
+CAB and +FLD; 8 wires run between a cabinet device and a field device; 15 symbols at the start).

## Replay

crates/novo-app/src/app/lesson_verify_terminals.rs `terminal_strips_steps_work_in_the_window` starts the shipped lesson
(`start_lesson`, so its own project.json opens) and carries out every step through the window: key events (Enter, Tab,
Ctrl+3, Up), Ctrl+K with typed words (`type_command`), real pointer clicks at the screen position of the terminal body
(`world_to_screen`), a Shift+click, right clicks and a click on the painted context-menu row (found by its painted text).
The lesson's own runner has to tick each step from its `expect` (a step that ticks before the action, or never, fails).
Not driven: hover tooltips. The "Show me how" pointer is not driven either.

## Verified path per step (paths under crates/)

1. goal: reading. Location comes from the tag (`+CAB-U1`); proposals from novo-terminals/src/suggest.rs `suggest_terminals`:
   a wire with a device pin at BOTH ends in different locations (a wire that starts at a potential feed gets none).
2. accept-one: Enter = proposal.accept (novo-workflow keymap.rs) -> novo-app app/terminals.rs `accept_proposal`; check `device -X1`.
3. next-proposal: Tab = proposal.next -> terminals.rs `next_proposal` (highlight index 0 to 1 asserted); check `command`.
4. accept-all: `proposal.accept_all` has NO key; Ctrl+K "accept all" -> terminals.rs `accept_all_proposals`; check `count symbol >= 23`.
5. open-drawer: Ctrl+3 -> workbench.rs `toggle_drawer`; check `command view.terminals_drawer` (no anchor, see GAPs).
6. read-table: reading; heads asserted painted (terminals.rs STRIP_HEADS).
7. select-two: click the BODY of terminal 5, Shift+click terminal 7 (their wires both carry 0V, asserted); check `selected_count 2`.
   REFUSED once: a click on the terminal origin is its end pin and starts a wire; the first drawing of the lesson aimed there.
   The `point` areas now cover the body (origin + 0..8 mm below).
8. connect: right click -> row "Connect terminals" (novo-editor view.rs context menu) -> terminals.rs `connect_terminals`;
   asserts jumper J1 on 5 and 7 and the drawer cells (Jumper `J1`, Bridged to `7`).
9. toggle-pe: click the PE terminal, Ctrl+K "pe", Enter -> terminals.rs `toggle_pe`; the Kind cell changes from PE.
10. reset-pe: right click -> row "Reset PE override" (only after a hand choice) -> `reset_pe_override`; kind PE again.
11. move-strip: click the PE terminal, Up -> `terminal.strip_up` -> move_strip.rs `move_to_strip`; check `terminal strip -X2 name PE`.
12. done: reading; ends with what to compare in finished.json.

## GAPs closed since the first version
- check kind for a terminal: `terminal = { strip, name }` exists now and is used in move-strip.
- `select` solve op has `add = true` (still no terminal designation, see below).

## GAPs still open
- solve `select = "-X1:5"` (a terminal designation) is not understood by the engine (select takes a device tag): `# GAP solve` in select-two, toggle-pe, move-strip.
- No anchors for the drawers, the table, the canvas menu rows or the palette field: the drawer steps have no pointer and check the command, the palette step points at `palette.button`.
- BUG: terminal.connect, terminal.disconnect and terminal.strip_up/down/lowest are not in APP_COMMANDS (app/workbench.rs), so the command field lists them greyed as coming later although keys and menu run them. The lesson names the right-click row for Connect.
- No check kind for "a jumper group is set": the command event stands in.
- The Terminals drawer shows 7 of the 8 rows without scrolling at the 1600x900 test size (row 7 is below the fold); the text only names rows 5 and 1 to 6.
- The same-name (join) rule cannot be applied from the editor (rename refuses a taken name, `place_named` is script-only, `rule_sentence` is shown nowhere): not taught (glossary `same-name-rule`).

## How lists

The `how`, `how_nl` and `demo` lists are in lesson.toml (reading steps have none).
