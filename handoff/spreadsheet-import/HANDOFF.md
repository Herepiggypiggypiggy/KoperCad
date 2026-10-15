# Spreadsheet import: handoff

Branch claude/project-thread-zen610 (kopercad), PR #411, based on #414 (claude/project-thread-k6t1zj) until that merges, then rebase onto main. My first planner (sheet/) was dropped in favour of #414's `sheets`.

My additions: crates/novo-interop/src/sheets/create.rs (NewDevice, PartRef, Ids), small hooks in plan.rs (new_devices, plan_import_with, command inserts) and fields.rs (Parsed::Part, Order number editable). Tests: tests/sheets_create.rs. Doc: docs/design/spreadsheets.md section "New devices and parts".

Traps: new devices only when the sheet has an Id column. One id counter (Ids) for parts and devices so Insert ids match run order. Round-trip thread owns the rest of sheets/; keep hooks small to avoid conflicts.

## How to reach it (UI entry)
Not done until the import dialog (thread session_0152EZHJQEhZVhJcXFt5h8az, app + `kopercad-cli import sheet`) shows: new devices (`plan.new_devices`, toggle `select_new`) in the preview with a "New" filter, and Order number part changes (ordinary `Change` rows, header "Order number"); the dialog must call `plan_import_with(project, &workbook, &catalog)` so parts resolve via the catalog. Until that lands this work is engine-only.
