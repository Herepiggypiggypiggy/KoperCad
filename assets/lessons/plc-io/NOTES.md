# plc-io: reviewer notes

Teaches: add a PLC card, send a button (DI) and a lamp (DQ) to it, read the addresses in the I/O list,
renumber the I/O, then try both in the live view. Status `draft` (anchors, live-view checks and a few UI gaps below).
Builder and tests: `crates/novo-tests/tests/lessons/plc.rs` (start, finished, I/O list rows, renumber preview).
Live behaviour was checked once with a scratch novo-sim test on finished.json (not kept, novo-tests has no novo-sim dependency):
rest = nothing; button closed = 1 input seen, lamp off; output on = lamp on.

Verified UI path per step (all `# verified:` comments in lesson.toml carry the same line numbers)
- add-card: Ctrl+K (keymap.rs:313), command field (command_field.rs:184-197), verb "add card" (commands.rs:303-310), handler plc.rs:34, message plc.rs:64. Needs the layout typed: no chips.
- select-button / select-lamp: plc.to_plc needs one selected symbol (workbench.rs:252, plc.rs:72-80).
- button-to-plc / lamp-to-plc: verb "to plc" (commands.rs:313-321), plc.rs:72-148; DI/DQ by symbol function; channel lands 50 mm right.
- io-list: "page 30" (palette.rs:220/544); the I/O list page is in the start project; live report (reports.rs:160).
- renumber: verb "renumber io" (commands.rs:414), plc.rs:154-252 (preview "Renumber (Enter)" / "Cancel (Esc)").
- live-on: Shift+L (keymap.rs:373), live.rs:362, card live.rs:658-690. press-button: pole badge live.rs:515-556, input dot live.rs:600-624.
- switch-output: square badge on the DQ, tooltip "<tag> · output off · click to switch it on" (live.rs:558-598, 870-875); novo-sim plc.rs puts 24V on the pin.

The start gap: the cabinet already has analog card -K1 (AI4, slot 1) starting at input byte 4, so `renumber io` has two moves (%I4 to %I0, %I12 to %I8).
Without such a gap the command only says "I/O addresses are already in order" (plc.rs:170).

GAPs (owner)
1. No address, signal text or channel label drawn at a channel symbol; no card symbol/overview; Devices drawer does not expand cards; no inspector chip; `plc.signal` has no editor (WS-17 with WS-03/WS-10 labels, WS-01 drawer/inspector). The I/O list page is the only place to read them.
2. `add card` draws nothing, sets no description or part, and offers no argument chips (WS-17/WS-08).
3. Live-view badges have no anchors and no check kinds; solve has no operation for them or for pressing Enter on the renumber preview (Learning modules, WS-01).
4. `field` check with `equals` is not in design section 5 (Learning modules).
Oddities: to_plc wire retraces the symbol's own lead (lamp's lower pin) and, for sideways signals, cuts through the DQ box; the I/O list page appears only on open, so a project that gets its first card in a session never shows it until reopened.
