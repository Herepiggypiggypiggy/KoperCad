# Spreadsheet round trip (`novo-interop::sheets`): roadmap

Scope: export devices, terminals, wires and cables to xlsx/CSV, bulk edit, import
back with preview, conflicts, one undo step and a rejected-row report. Pure Rust,
no Excel install, no admin. Design: `docs/design/spreadsheets.md`.

## Done
- Engine (PR 1): `grid`, `fields`, `export`, `files` (xlsx + CSV), `plan`; tests in
  `crates/novo-interop/tests/sheets.rs`.

## Next, in order
1. `kopercad-cli export sheets` / `import sheets` (preview by default, `--apply`), thin.
2. App dialog: File menu export, import with preview table and conflict choice.
3. Cable cores sheet (core name, cross-section, shield; key = cable Id + core index).
4. New rows (add devices/terminals) with an explicit "add new rows" switch.
5. Rejected rows written as a workbook to fix and re-import.
6. Python: `project.export_sheets()` / `import_sheets()` in novo-py.

## Open questions
- Terminal Kind and PE override editable? (Left read-only: PE is rule-driven.)
- Should the parts list and I/O list (WS-13/WS-17) join the same workbook?
