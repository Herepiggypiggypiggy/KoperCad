# Spreadsheet cores: handoff

Branch `claude/project-thread-a2l1n9`, stacked on #414 (`claude/project-thread-k6t1zj`, thread cse_01ToRihU88DzNsNUPssCav6H owns `sheets`). Rebase onto main when #414 merges.

Where: `crates/novo-interop/src/sheets/{grid,fields,plan,files,export}.rs` (TableKind::Cores, Slot::Core*, `RowRef`, `Change.core`), tests in `tests/sheets_cores.rs`, format in `docs/design/spreadsheets.md` ("Cores").

Key decisions: a core has no id, so its key is the cable Id plus `Core No` (position); spare marks are stored by core name in `cable.spare` and follow a rename; wire numbers are edited on the Wires sheet only.

The #411 `sheet` module (devices, new rows) overlaps #414's Devices table; the lead decides which stays.
