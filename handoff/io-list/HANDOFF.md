# Pages from an I/O list: handoff

Branch `claude/project-thread-lp4ig7` (kopercad). Code: `crates/novo-plc/src/generate/{list,plan,apply}.rs`, tests `crates/novo-plc/tests/generate.rs`, app `crates/novo-app/src/app/plc_list.rs` + `plc_list_tests.rs`, command `plc.import_list` (workflow commands/menus/language, workbench dispatch).

State: engine and UI done, tests green; see PROGRESS.md for the PR.

Resume: `cargo test -p novo-plc -p novo-workflow`, then `cargo test -p novo-app plc_list`.

Decisions and traps:
- A channel is identified by `plc.field` (the list's tag). Re-importing skips tags already present; that is how a changed list adds only the new signals.
- Addresses are still derived from cards (plc.md 3). Addresses in the list only choose the card start and the spare gaps; if the group's addresses overlap an existing card, or only some rows have one, the whole group falls back to assigned addresses and a note says so.
- Cards with an explicit start are added first so a card without one never takes their range.
- Channels are placed with `place_symbol` and `plc.channel` set by number (not `place_channel`), because spares in the middle make "next free channel" wrong.
- Terminals are placed directly (not cut into a wire), so the field side stays unconnected and no open-wire problems appear.
- Row numbers in the notes count non-empty rows (csvio drops blank rows).
