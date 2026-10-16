# terminal-strips: notes for the reviewer

Teaches: accept terminal proposals (Enter, Tab, accept all), read the Terminals drawer, bridge two terminals
(Connect terminals), switch PE by hand and give it back, move a terminal to strip -X2 (Up). 12 steps, about 8 minutes.
Project: crates/novo-tests/tests/lessons/terminals.rs `terminal_strips_start` / `_finished` (pump, converter, two valves;
+CAB and +FLD; 8 wires cross the boundary; 15 symbols at the start). Status stays `draft`.

Verified path per step (all paths under crates/):
1. goal: reading. Location comes from the tag (`+CAB-U1`); proposals from novo-terminals/src/suggest.rs:263 `suggest_terminals`.
2. accept-one: Enter = proposal.accept (novo-workflow keymap.rs:370) -> novo-app app/terminals.rs:30; key only claimed while the page has terminal proposals (app/workbench.rs:370).
3. next-proposal: Tab = proposal.next (keymap.rs:371) -> terminals.rs:85. Dashed marks + note: terminals.rs:276.
4. accept-all: `proposal.accept_all` has NO key; Ctrl+K verb "accept all" (commands.rs:333) -> workbench.rs:503 -> terminals.rs:114.
5. open-drawer: Ctrl+3 (keymap.rs:391) -> workbench.rs:512; tab app/drawers.rs:61.
6. read-table: columns terminals.rs:183; hover texts terminals.rs:228/239.
7. select-two: Shift+click (novo-canvas selection.rs:17).
8. connect: right-click row "Connect terminals" (novo-editor view.rs:1744) -> terminals.rs:357 -> novo-terminals strip.rs:326.
9. toggle-pe: verb "pe" (commands.rs:446) -> terminals.rs:363 -> earth.rs `toggle_pe` (also menu row view.rs:1717).
10. reset-pe: menu row "Reset PE override" only after a manual choice (view.rs:1722); Ctrl+K row too -> terminals.rs:375.
11. move-strip: Up/Down/Ctrl+Up (keymap.rs:365-367) -> terminals.rs:385 -> move_strip.rs:80 `move_to_strip` (creates -X2).
12. done: reading.
Numbering as taught: lowest free number, left to right on vertical wires (suggest.rs `accept_suggestions`, place.rs `place_terminals`,
naming.rs `next_free_number`); a PE net gives the name PE (naming.rs:55). Tests: `terminal_strips_steps_hold_on_the_finished_project`.

GAPs
- BUG: terminal.connect, terminal.disconnect, terminal.strip_up/down/lowest are not in APP_COMMANDS (app/workbench.rs:181), so Ctrl+K shows them greyed "Comes later" (command_field.rs:82) although keys and the menu run them.
- No anchors exist yet (see the table in the final report). Solve needs: select by terminal designation, additive select.
- The same-name (join) rule cannot be applied from the editor: rename refuses a taken name (novo-editor edit.rs:333), `place_named` is called by scripts only, and `rule_sentence` is shown nowhere. Not taught; glossary term `same-name-rule` states it.
- No suggestion on a PE wire that starts at a potential feed (only device pins count); the sample's PE wire runs between the converter's PE2 pin and the motor for that reason.
