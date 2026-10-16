# wire-numbers: reviewer notes

Teaches: a wire's potential comes from its net, its number is filled, fixed or typed by you. Wire list, Wire numbering window (scheme and preview), `number wires`, `number wire 100` (locks one net), `renumber wires`, and the numbers on the cable diagram. Status stays `draft` (GAPs below).
Project: the pump station of `cables-and-shields` with both cables and two motor wires numbered 1 and 2 by hand. `finished.json` = every net numbered 1 to 8 plus 100 on the level loop; the verifier asserts the steps leave exactly these numbers. Builder: `crates/novo-tests/tests/lessons/cables.rs` (`WIRE_NUMBERS_FINISHED`).
Headless proof: `crates/novo-app/src/app/lesson_verify_cables.rs` (`wire_numbers_each_instruction_works_in_the_app`: Ctrl+K text, Ctrl+1, clicks on the window buttons by label and on the wire; `..._solve_scripts_tick_every_step`).

| Step | Instruction and verified path (file:line) |
| --- | --- |
| open-wire-list | Ctrl+K `wire list` (`commands.rs:1139` report.wires), `reports_view.rs:80` switches to the Reports view |
| read-list | columns `novo-reports wires.rs:191-206`; the test reads 1, 2 and eight empty |
| back-to-drawing | Ctrl+1 `keymap.rs:389` view.schematic |
| preview | `wire numbering` `commands.rs:681` -> `workbench.rs:657` -> window `numbering.rs:178`; plan "7 numbered, 2 settled, 0 unchanged"; window closes with its **Close window** button (Esc did not close it when tried by key) |
| number-wires | `wires.number` `commands.rs:672` -> `numbering.rs:75`; one undo step; duplicates would be renumbered (not in the start project), unique numbers kept |
| type-number | click the wire, `number wire 100` (`commands.rs:706`, `numbering.rs:92`): `number wire 1` is refused first ("1 is already used on another net", `manual.rs:20-31`), then the net takes 100 and is locked |
| renumber | `renumber wires` `commands.rs:691` opens on Selection when a wire is selected, so **Project** then **Renumber** (`numbering.rs:233-246`); toast "8 renumbered, 1 unchanged" |
| wire-list-again, cable-diagram | `report.wires`, `report.cables` (`commands.rs:1139`, `:1148`); cable diagram columns `diagrams.rs:185` |

Closed since the first writer: numbering rule, number wires, renumber, first number of an unnumbered wire (`number wire`), duplicate detection (the window's plan and `check wire numbers`; `check` reports duplicates before a run, a Fill plan shows none because it repairs them).

GAPs still open
- No check kind reads a wire's number: the checks are the commands (`wires.number`, `wires.set_number`, `wires.renumber`); the `renumber` step ticks when the window opens, before **Renumber** is clicked, so it is a `pause` step. Wished: `wire_number = { wire, is }`, solve `numbering = "renumber"`, a solve `select` for a wire.
- No `wire.number.*` design check yet (docs/design/wire-numbering.md "Later").
- The old step that renamed the supply potential (right-click feed, chip) is dropped: it cannot be driven headless and is a potentials lesson topic.
- `check wire numbers` opens the window on the Conflicts tab, which lists only what remains after a run; duplicates are visible in the Preview tab.

The `how`, `how_nl` and `demo` lists are in lesson.toml.
