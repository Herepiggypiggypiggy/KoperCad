# Spreadsheet cores, bridges and messy-sheet tests: roadmap

Scope: extend the #414 round trip (`novo_interop::sheets`) to cable cores, jumper bridges and real-world messy files.

Done
- Cores sheet: export and previewed import (core name, section, shield, spare, wire id); reorder guard; clash warnings. Stacked on #414.

Next
1. Terminals: `Bridged with` orientation column, warning for a jumper group with one terminal.
2. Messy-sheet round-trip tests (Excel numbers as numbers, apostrophe tags, semicolon CSV, reordered and extra columns, duplicate rows, trailing spaces).
3. New cores and new terminals (needs the "new rows" decision with the export thread).
