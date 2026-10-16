# cables-and-shields: reviewer notes

Teaches: wires that leave the cabinet become cables; the Cables drawer; the C tool; accepting proposals; reading cores; and, honestly, what a cable can hold that the screen cannot set yet (part, length, shield). Status stays `draft` (GAPs below).
Project: pump station (motor and level transmitter in +FLD, terminal strip and PLC input in +CAB), built with `novo_editor::edit::place/draw_wire`. `finished.json` = both cables plus part, length and shield set through the engine (`assign_cable_part`, `add_shield`, `connect_shield`); the last step says so. Builder: `crates/novo-tests/tests/lessons/cables.rs`.
Headless proof: `crates/novo-app/src/app/lesson_verify_cables.rs` (`cables_and_shields_each_instruction_works_in_the_app`: real Ctrl+4, C, two clicks on the page, Enter, a click on the drawer button by its label; `..._solve_scripts_tick_every_step`: the solve scripts). The running lesson's own checker ticks each step.

| Step | Instruction and verified path (file:line) |
| --- | --- |
| open-drawer | Ctrl+4 `keymap.rs:392`, handler `workbench.rs:670`; tab and button `drawers.rs:62`, `:76`. Check is the command (no `drawer.cables` anchor exists) |
| read-proposals | 2 proposals (4 + 2 wires), count `drawers.rs:51`; empty text `drawers.rs:185` |
| cable-tool | C `keymap.rs:348`, `workbench.rs:601`, `tool.rs:696`; dock has no Cable button |
| draw-line | clicks (80,116) and (104,116), Enter = `proposal.accept` `keymap.rs:370`, `tool.rs:1904`, `tool.rs:1730`, `drawers.rs:128`; toast "Cable =P1-W1: 3x1.5 on 2 wires" |
| accept | button "Accept 1 proposed" `drawers.rs:76` -> `drawers.rs:153`; toast "Accepted 1 cable: =P1-W2 4G1.5" |
| read-cores | core grid `drawers.rs:224-262`, headers are the end locations +CAB / +FLD (`side_head`); BN L1, BK L2, GY L3, GNYE PE (test asserts) |
| free-core | list click `drawers.rs:209`; "2 of 3" `drawers.rs:214` (the test asserts the overview row, the cell is not in the accessibility tree) |
| part-length-shield | reading; Cables sheet "Length [m]" `novo-interop sheets/fields.rs:213` |

GAPs still open
- Cable proposals are not drawn on the page, have no command id or key (Enter accepts terminal proposals only) and cannot be declined; the drawer button is the only way.
- No screen control sets a cable's part, type or length, or adds/connects a shield (`assign_cable_part`, `add_shield`, `connect_shield` are not called from novo-app); `accept_proposals` gets no catalog (`drawers.rs:163`). The lesson says so in one reading step.
- No Cable button in the dock. The generated Cable diagram page appears only when a project opens.
- Wished for: check kinds `field` on a cable (part, length) and `cable` with `shield_connected`; solve ops `cable_part`, `cable_shield`, a click on a control; anchors `drawer.cables` / `drawer.cables.accept` (until then the checks are commands).

## Pending how
The `how` / `how_nl` fields are not in the lesson parser yet; paste these per step id when they land.

- open-drawer
  - how: ["Press {key:view.cables_drawer}", "The drawer opens under the drawing with the tabs **Terminals**, **Cables** and **Devices**; **Cables** is selected"]
  - how_nl: ["Druk op {key:view.cables_drawer}", "De lade opent onder de tekening met de tabbladen **Terminals**, **Cables** en **Devices**; **Cables** is geselecteerd"]
- cable-tool
  - how: ["Move the pointer over the drawing", "Press {key:tool.cable}; the status line says the Cable tool is active"]
  - how_nl: ["Beweeg de aanwijzer over de tekening", "Druk op {key:tool.cable}; de statusregel meldt dat het gereedschap Kabel actief is"]
- draw-line
  - how: ["Click on empty page at the left of the first transmitter wire, about halfway between terminal 4 and -B1", "Click again at the right of the second wire, so the line crosses both", "Press {key:proposal.accept} to finish; the toast reads Cable =P1-W1"]
  - how_nl: ["Klik op lege pagina links van de eerste draad van de zender, ongeveer halverwege klem 4 en -B1", "Klik opnieuw rechts van de tweede draad, zodat de lijn beide kruist", "Druk op {key:proposal.accept} om af te ronden; de melding zegt Cable =P1-W1"]
- accept
  - how: ["In the drawer, click **Accept 1 proposed**", "The list on the left now shows two cables"]
  - how_nl: ["Klik in de lade op **Accept 1 proposed**", "De lijst links toont nu twee kabels"]
- free-core (reading)
  - how: ["Click **=P1-W1** in the list on the left", "Read **2 of 3** in its Cores column"]
  - how_nl: ["Klik links in de lijst op **=P1-W1**", "Lees **2 of 3** in de kolom Cores"]
