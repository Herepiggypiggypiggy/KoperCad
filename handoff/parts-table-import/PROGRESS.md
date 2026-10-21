# Real part data import: progress (append-only)

2026-10-21 18:56 UTC Thread started from the coordinator: real part data import with preview, mapping, validation, honest labels.
2026-10-21 19:10 UTC Read gap analysis, PARTS-BULK-IMPORT.md and novo-parts: BMEcat/CSV/inbox and Parts quality already exist; missing: user mapping, Excel, preview before commit.
2026-10-21 19:40 UTC Engine written in novo-parts: tablefile.rs, tablemap.rs, tablepreview.rs; 13 integration tests with a real generated xlsx.
2026-10-21 20:00 UTC App dialog parts_table.rs, command parts.import_table, menu File > Import parts (the Import submenu was at its 12-item cap, so the three parts imports moved into their own submenu), Parts window button, .xlsx drop.
2026-10-21 20:15 UTC QA found two real defects before PR: a comma in a title line broke delimiter detection (now voting over 30 lines, test added); header row guess picked the title (now width based, test added).
2026-10-21 20:30 UTC QA scenario 21-parts-table-import green at 1366x768 and 1920x1080; evidence in /mnt/project-files/qa/parts-table-import/.
