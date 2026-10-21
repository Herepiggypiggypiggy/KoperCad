# Spreadsheet memory bound: handoff

Branch `claude/data-safety-xlsx-bound` (own PR from main; the data-safety thread's other work is `handoff/data-safety/`, merge the two folders after both land).
Code: `crates/novo-parts/src/xlsx_guard.rs` (`check_xlsx`, `XlsxLimits::TABLES`), called first by `novo-parts` tablefile, `novo-plc` link/read.rs and generate/list.rs, `novo-interop` sheets/files.rs (`read_xlsx`) and spec.rs (`read_grid`). `novo-plc` now depends on `novo-parts`.
Tests: unit tests in the guard, `crates/novo-tests/tests/xlsx_bounds.rs` (crafted workbooks of a few hundred bytes through every reader).
Rule: any new `.xlsx` reader must call `check_xlsx` before `open_workbook_from_rs`.
Limits: 200,000 rows, 8,000,000 cells in the used rectangle, 256 MiB unzipped; the scan reads the sheet XML once as a stream. It does not look at cell contents or shared strings beyond the unzipped-size cap.
2026-10-21 21:50 UTC guard added, wired into five readers, tests pass.
2026-10-21 21:40 UTC Reviewer A: guard now parses with quick-xml (local_name, attribute iterator); tests for prefix, single quotes, spaced =.
2026-10-21 21:50 UTC Review round 2: real decompressed bytes counted (header lies refused), saturating sums and rows, fail closed on unreadable parts, all .xml parts scanned.
