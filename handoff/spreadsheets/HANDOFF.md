# Spreadsheet round trip: handoff

## State (2026-10-15 18:50 UTC)
- Branch `claude/project-thread-k6t1zj`, draft PR "Spreadsheet round trip: engine"
  against main. Engine, tests and design doc are in; no UI/CLI yet.
- Nothing half-done.

## What exists
- `crates/novo-interop/src/sheets/`: `grid.rs` (Cell, Table, Workbook), `fields.rs`
  (columns, read/parse/apply per column), `export.rs` (`export_project`), `files.rs`
  (`xlsx_bytes`, `read_xlsx`, `to_csv`, `read_csv`), `plan.rs` (`plan_import`,
  `ImportPlan`, `apply`).
- Hidden sheets `~Devices` etc. hold the baseline; `~Meta` holds format, project, revision.

## How to reach it (UI)
- File > Export spreadsheet (.xlsx) and File > Export spreadsheet (CSV files); palette `export.sheets` / `export.sheets_csv` ("export excel", "excel", "export csv").
- Code: crates/novo-app/src/app/sheets.rs (thin; writes next to the project, only replaces files it wrote). Import dialog + CLI: dialog/CLI thread (session_0152EZHJQEhZVhJcXFt5h8az).

## Resume
```
cargo test -p novo-interop --test sheets
cargo clippy -p novo-interop --all-targets -- -D warnings
```

## Decisions and traps
- Match by Id; tag/name fallback only when there is no Id column value.
- Row-level atomic: one bad cell rejects the whole row; conflicts are cell-level.
- Apply is one `Command::Batch` of `Replace` records; refuses a stale plan (revision check).
- Tags are text cells; CSV puts `'` before `=`/`-` text and the reader removes it.
- Only dependencies already in `[workspace.dependencies]` were used (calamine, rust_xlsxwriter).
- Reviewer: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W; lead cse_01FiLvRBKEcNNHbQEGhYyL1S.
