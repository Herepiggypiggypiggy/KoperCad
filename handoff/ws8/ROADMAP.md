# WS-08 Keyboard workflow: roadmap

Scope: the engine behind the command field, keys and fast edits, in the crate
`crates/novo-workflow` (pure logic, no egui). Spec:
`docs/DESIGN.md` (merged on main; keys table, command surface). EPLAN complaints are symptoms, not a
feature list (Tadjiro: "its own program, not an EPLAN copy").

## Done (PR #29)
- Keymap per interaction-model section 7, focus scopes (Canvas, PlaceFieldEmpty, TextField, Panel), conflict test, and a test that no typing key is bound outside the canvas
- Command registry grouped Project/Draw/Edit/Navigate/View/Check/Export, with verbs, argument chips and disabled reasons
- Command field engine (palette.rs): noun jumps (exact tag gives the device plus every /page.column location, via novo-core Project::search and cross_references), verb + noun runs, alias + device places a child (`no K1`), bare verb runs; an empty field is the grouped menu with Repeat last tool first; place mode (I) shows favourites; row `completion` makes every form pickable without typing
- Symbol search via novo-symbols Library::by_alias / favorites(); IEC 81346-2 letter codes; next_tag
- Renumber (project/page/devices, preview, one undo step); smart duplicate/paste (next free tags, coil and contacts together, terminals continue the strip)
- Jump history (Alt+Left/Right); R/X orientation table

## Next, in order
0. Follow-up PR from main after #29 merges (DESIGN.md section 4 is the reference): rename view.focus_mode to view.focus and add view.pin_outline to match WS-01; verbs take English + Dutch alias lists per word (DESIGN section 3, PR #43), with tags and IEC codes never translated; the app maps logical keys so ? and Ctrl+Y work on QWERTZ and Dutch layouts; the canvas consumes Tab; Repeat last tool has no key (already true); keys DESIGN.md adds that #29 lacks are Shift+O (held, override marks) and ? (shortcut overlay). DESIGN shows Ctrl+Y as the primary redo key, while #29 shows Ctrl+Shift+Z to match the app shell; agree this with WS-01.
1. After #29 merges: a new PR from main that wires the engine into novo-app (palette.rs calls Palette::search_in, shortcuts go through Keymap::resolve). WS-01 agreed and won't touch palette.rs or actions.rs. Model edits go through the backend (OpenPage / backend.record, see WS-01 #34). Keep the UI thin until DESIGN.md merges.
2. `accept all` summary row ("9 terminals on -X1, 3 cables") with a second Enter, once proposals exist (WS-06/WS-11).
3. Linked blocks: `dol 4kW x3` once WS-02 has BlockDefinition/BlockInstance.
4. Problems list engine (`problems`, F8 order) once checks exist (WS-14).

## Open questions
- Final key names and drawer tabs follow DESIGN.md (current draft: Ctrl+3/4/5 Terminals/Cables/Devices, Ctrl+Shift+F focus, F11 full screen).
