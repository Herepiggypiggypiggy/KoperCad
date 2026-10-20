# EPLAN-SYMBOLS handoff
Branch claude/project-thread-rwdajk (kopercad). Importer owner: thread cse_01MiV9V7WMLKufpLNco1LoDc.
Boxes on the real export (133): device connection point 66, terminal with 3 points 35, black box 13 (left a box on purpose), female pin 9, variable motor 4, triple switch 2, wire end 2, transformer with PE 1, CEE socket 1.
Added: connectivity/device_pin, wire_end_insulated; terminals/terminal_tap, socket_contact, plug_contact, socket_3p_pe, socket_3p_n_pe, plug_3p_pe, plug_3p_n_pe; motors/motor_3ph_pe_t2 (T1/T2 aux terminals: auxiliary terminals, function unknown; inferred from pin positions); transformers/transformer_1ph_pe. Rows in `crates/novo-interop/src/eplan/symbols/table.toml`.
`map_fitting(source, ports)` picks a family member by connection count when pins have no names. `fit_geometry` (symbols/geometry.rs) gives rotation, mirror and pin order from positions.
Table rows alone take the export from 133 boxes to 60. Terminals (44) and the transformer need draw.rs to call the mapper for terminal placements and `map_fitting`. Possible importer bug to check: box pins store direction with up/down swapped (top pin faces down).
No EPLAN artwork or library used.
