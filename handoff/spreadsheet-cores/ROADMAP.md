# Spreadsheet cores, bridges and messy-sheet tests: roadmap

Scope: extend the #414 round trip (`novo_interop::sheets`) to cable cores, jumper bridges and real-world messy files.

Done
- Cores sheet: export and previewed import (core name, section, shield, spare, wire id); reorder guard; clash warnings. Stacked on #414.

Done too
- Terminals: `Bridged with` orientation column, lone-jumper warning. Messy-sheet tests in `tests/sheets_messy.rs`.

Done (FU-SHEET, branch claude/project-thread-v3fqdt)
- New terminal, cable and core rows on import (`sheets/new_rows.rs`, tests `tests/sheets_new_rows.rs`).

Next
1. App dialog lists `ImportPlan::added_rows()` and uses `select_added` (dialog thread; sheet_import.rs still shows only new devices and its empty-plan check ignores new rows).
2. Removals of cores/terminals (not decided).
