# plc-io: reviewer notes

Teaches: add a PLC card, send a button (DI) and a lamp (DQ) to it, read the addresses in the I/O list, close the address gap with Renumber I/O, then try both in the live view. Status stays `draft` (GAPs below).
Project: 24 V pump circuit, start button -S1, running lamp -H1, an analog card -K1 starting at input byte 4 (so renumbering has two moves). `finished.json` = after the steps. Builder: `crates/novo-tests/tests/lessons/plc.rs`.
Headless proof: `crates/novo-app/src/app/lesson_verify_cables.rs` (`plc_io_each_instruction_works_in_the_app`: Ctrl+K text, clicks on the symbols, Enter on the preview, Shift+L, real clicks on the pole and output badges at their painted positions; `plc_io_solve_scripts_tick_every_step`).

| Step | Instruction and verified path (file:line) |
| --- | --- |
| add-card | Ctrl+K `add card DI16 DQ8` (`commands.rs:457`) -> `plc.rs:34`; toast "Added card =A1+CAB-A1 (DI16 DQ8)"; nothing drawn (page keeps 4 symbols) |
| select-button | click the symbol (hit region found by hovering) |
| button-to-plc, lamp-to-plc | `to plc` (`commands.rs:466`) -> `plc.rs:72-148`; DI at (90,44), DQ at (170,48); toasts "Sent to =A1+CAB-A1: next free DI/DQ channel"; page 4 -> 5 -> 6 symbols |
| io-list | `page 30`; rows %IX12.0 DI "Pump start button", %QX0.0 DQ "Pump running lamp" |
| renumber | `renumber io` (`commands.rs:588`) -> `plc.rs:155`, dialog `plc.rs:190` "Renumber (Enter)"/"Cancel (Esc)"; moves (4 -> 0), (12 -> 8); toast "Renumbered I/O: 2 start address(es) moved"; list then reads %IX8.0 |
| back-to-drawing | `page 1` |
| live-on | Shift+L `keymap.rs:373`; card with **Reset** and **PLC program…** (the button was called All open before main changed the card) |
| press-button | pole badge at (48,44), `live.rs:262-290`; click closes the pole; check `pole_closed` (replaces the `clicked` check, no anchor exists) |
| switch-output | output badge at (170,51), `live.rs:306-336`; check `lit = "-H1:X2"` (false until the output is on) |

Re-checked on current main (first writer's report): PLC channels still show no address on the drawing (open); `add card` still draws no card symbol (open); the forced output is still switched by clicking the square badge, now taught as the way and with the **PLC program…** button mentioned; cable/live anchors still unregistered.

GAPs still open
1. No address, signal text or channel label at a channel symbol; no card symbol; Devices drawer does not expand cards; no inspector chip; `plc.signal` has no editor (WS-17 with WS-03/WS-10, WS-01).
2. `add card` sets no description or part and offers no argument chips.
3. Solve cannot press Enter on the renumber preview (wanted `arg = "apply"`) nor click a live badge (wanted `live_click`); the replay test presses/clicks them itself, and the two live steps use `select` as stand-in solve. No anchors for the badges.
4. The I/O list page is added when a project opens; a first card added in a session shows it after reopening.
Oddity: the `to plc` wire for a lamp retraces 4 mm of the lamp's lead.

The `how`, `how_nl` and `demo` lists are in lesson.toml.
