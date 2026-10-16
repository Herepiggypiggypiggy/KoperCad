# plc-io: reviewer notes

Teaches: add a PLC card, send a button (DI) and a lamp (DQ) to it, read the addresses in the I/O list, close the address gap with Renumber I/O, then try both in the live view. Status stays `draft` (GAPs below).
Project: 24 V pump circuit, start button -S1, running lamp -H1, an analog card -K1 starting at input byte 4 (so renumbering has two moves). `finished.json` = after the steps. Builder: `crates/novo-tests/tests/lessons/plc.rs`.
Headless proof: `crates/novo-app/src/app/lesson_verify_cables.rs` (`plc_io_each_instruction_works_in_the_app`: Ctrl+K text, clicks on the symbols, Enter on the preview, Shift+L, real clicks on the pole and output badges at their painted positions; `plc_io_solve_scripts_tick_every_step`).

| Step | Instruction and verified path (file:line) |
| --- | --- |
| add-card | Ctrl+K `add card DI16 DQ8` (`commands.rs:457`) -> `plc.rs:34`; toast "Added card =A1+CAB-K2 (DI16 DQ8)"; nothing drawn (page keeps 4 symbols) |
| select-button | click the symbol (hit region found by hovering) |
| button-to-plc, lamp-to-plc | `to plc` (`commands.rs:466`) -> `plc.rs:72-148`; DI at (90,44), DQ at (170,48); toasts "Sent to =A1+CAB-K2: next free DI/DQ channel"; page 4 -> 5 -> 6 symbols |
| io-list | `page 30`; rows %IX12.0 DI "Pump start button", %QX0.0 DQ "Pump running lamp" |
| renumber | `renumber io` (`commands.rs:588`) -> `plc.rs:155`, dialog `plc.rs:190` "Renumber (Enter)"/"Cancel (Esc)"; moves (4 -> 0), (12 -> 8); toast "Renumbered I/O: 2 start address(es) moved"; list then reads %IX8.0 |
| back-to-drawing | `page 1` |
| live-on | Shift+L `keymap.rs:373`; card with **All open** and **PLC program…** |
| press-button | pole badge at (48,44), `live.rs:262-290`; click closes the pole; check `pole_closed` (replaces the `clicked` check, no anchor exists) |
| switch-output | output badge at (170,51), `live.rs:306-336`; check `lit = "-H1:X2"` (false until the output is on) |

Re-checked on current main (first writer's report): PLC channels still show no address on the drawing (open); `add card` still draws no card symbol (open); the forced output is still switched by clicking the square badge, now taught as the way and with the **PLC program…** button mentioned; cable/live anchors still unregistered.

GAPs still open
1. No address, signal text or channel label at a channel symbol; no card symbol; Devices drawer does not expand cards; no inspector chip; `plc.signal` has no editor (WS-17 with WS-03/WS-10, WS-01).
2. `add card` sets no description or part and offers no argument chips.
3. Solve cannot press Enter on the renumber preview (wanted `arg = "apply"`) nor click a live badge (wanted `live_click`); the replay test presses/clicks them itself, and the two live steps use `select` as stand-in solve. No anchors for the badges.
4. The I/O list page is added when a project opens; a first card added in a session shows it after reopening.
Oddity: the `to plc` wire for a lamp retraces 4 mm of the lamp's lead.

## Pending how
- add-card
  - how: ["Press {key:view.command_palette}", "Type **add card DI16 DQ8** and press Enter", "The toast reads Added card =A1+CAB-K2"]
  - how_nl: ["Druk op {key:view.command_palette}", "Typ **add card DI16 DQ8** en druk op Enter", "De melding zegt Added card =A1+CAB-K2"]
- select-button
  - how: ["Click the pushbutton symbol -S1 on the drawing; it shows as selected"]
  - how_nl: ["Klik op het drukknopsymbool -S1 op de tekening; het wordt geselecteerd"]
- button-to-plc
  - how: ["Press {key:view.command_palette}", "Type **to plc** and press Enter", "A DI box appears 50 mm right of -S1"]
  - how_nl: ["Druk op {key:view.command_palette}", "Typ **to plc** en druk op Enter", "Een DI-box verschijnt 50 mm rechts van -S1"]
- lamp-to-plc
  - how: ["Click the lamp symbol -H1", "Press {key:view.command_palette}, type **to plc** and press Enter", "A DQ box appears 50 mm right of -H1"]
  - how_nl: ["Klik op het lampsymbool -H1", "Druk op {key:view.command_palette}, typ **to plc** en druk op Enter", "Een DQ-box verschijnt 50 mm rechts van -H1"]
- io-list
  - how: ["Press {key:view.command_palette}, type **page 30** and press Enter", "Find the rows Pump start button and Pump running lamp"]
  - how_nl: ["Druk op {key:view.command_palette}, typ **page 30** en druk op Enter", "Zoek de rijen Pump start button en Pump running lamp"]
- renumber
  - how: ["Press {key:view.command_palette}, type **renumber io** and press Enter", "Read the preview: -K1 %I4 to %I0, -K2 %I12 to %I8", "Press Enter (or click **Renumber (Enter)**); Esc cancels"]
  - how_nl: ["Druk op {key:view.command_palette}, typ **renumber io** en druk op Enter", "Lees de voorvertoning: -K1 %I4 naar %I0, -K2 %I12 naar %I8", "Druk op Enter (of klik op **Renumber (Enter)**); Esc annuleert"]
- back-to-drawing
  - how: ["Press {key:view.command_palette}, type **page 1** and press Enter"]
  - how_nl: ["Druk op {key:view.command_palette}, typ **page 1** en druk op Enter"]
- live-on
  - how: ["Move the pointer over the drawing", "Press {key:sim.live}; a card Live potentials appears at the top left"]
  - how_nl: ["Beweeg de aanwijzer over de tekening", "Druk op {key:sim.live}; linksboven verschijnt de kaart Live potentials"]
- press-button
  - how: ["Click the round badge halfway between the two pins of -S1", "The wire lights up and the dot on the DI box fills"]
  - how_nl: ["Klik op het ronde knopje halverwege de twee pinnen van -S1", "De draad licht op en de stip op de DI-box wordt gevuld"]
- switch-output
  - how: ["Click the square badge just above the pin of the DQ box", "The lamp -H1 lights; **All open** on the card switches everything off again"]
  - how_nl: ["Klik op het vierkante knopje net boven de pin van de DQ-box", "De lamp -H1 brandt; **All open** op de kaart zet alles weer uit"]
