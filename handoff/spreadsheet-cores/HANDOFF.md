# Spreadsheet cores: handoff

Branch `claude/project-thread-a2l1n9`, stacked on #414 (`claude/project-thread-k6t1zj`, thread cse_01ToRihU88DzNsNUPssCav6H owns `sheets`). Rebase onto main when #414 merges.

Where: `crates/novo-interop/src/sheets/{grid,fields,plan,files,export}.rs` (TableKind::Cores, Slot::Core*, `RowRef`, `Change.core`), tests in `tests/sheets_cores.rs`, format in `docs/design/spreadsheets.md` ("Cores").

Key decisions: a core has no id, so its key is the cable Id plus `Core No` (position); spare marks are stored by core name in `cable.spare` and follow a rename; wire numbers are edited on the Wires sheet only.

The #411 `sheet` module (devices, new rows) overlaps #414's Devices table; the lead decides which stays.

## How to reach it (UI)
No separate command: the Cores sheet is part of the normal export (`export_project` writes a `Cores` sheet when a cable has cores) and import (`plan_import`) used by the export/import menu commands and the import dialog. In the preview, Cores changes have `table == TableKind::Cores`, `core == Some(index)` and `item` like `=A1-W1 core 2 (BN)`; rejected rows list sheet `Cores`. Dialog thread: session_0152EZHJQEhZVhJcXFt5h8az. Export: session_01ToRihU88DzNsNUPssCav6H.

## FU-SHEET update
New rows: Terminals (strip + name unknown), Cables (tag unknown), Cores (Core No one past the end or empty with cable tag). Code in `crates/novo-interop/src/sheets/new_rows.rs`, hooks in `plan.rs` (`plan_table`, `command`), `create.rs` (`Ids::insert_cable/insert_terminal`). API for a dialog: `ImportPlan::added_rows()`, `select_added(i, bool)`, `new_terminals/new_cables/new_cores`. Dependents (terminals of a new strip, cores of a new cable) are dropped when the parent is left out. Tables are planned in kind order so new rows can name new devices/cables. The old test `no_cores_are_added...` in sheets_cores.rs changed to `a_core_row_past_the_end_adds_a_core...`.
