# Spreadsheet cores, bridges and messy-sheet tests: roadmap

Scope: extend the #414 round trip (`novo_interop::sheets`) to cable cores, jumper bridges and real-world messy files.

Done
- Cores sheet: export and previewed import (core name, section, shield, spare, wire id); reorder guard; clash warnings. Stacked on #414.

Done too
- Terminals: `Bridged with` orientation column, lone-jumper warning. Messy-sheet tests in `tests/sheets_messy.rs`.

Next
1. New cores and new terminals (needs the "new rows" decision with the export thread).
