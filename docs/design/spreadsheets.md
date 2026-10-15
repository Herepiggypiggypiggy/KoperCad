# Spreadsheet round trip (`novo-interop::sheets`)

Status: engine implemented; app, CLI and Python entry points follow.

## What the designer does

Export the devices, terminals, wires and cables, change two hundred
descriptions or renumber a strip in Excel, save, import. The program shows
what would change, holds back what clashes with later edits, says which
rows it could not use and why, and applies the rest as **one undo step**.
No Excel install and no admin rights: the files are read and written by
pure-Rust code (`rust_xlsxwriter`, `calamine`).

Problems of the usual EPLAN route avoided: no import that silently
overwrites, no matching by tag (a rename in the sheet would create a new
item or fail), no cryptic "import failed" without row and cause.

## Tables and columns

One sheet (or CSV) per kind: **Devices**, **Terminals**, **Wires**,
**Cables**. Column `Id` comes first and is the key. Greyed columns (Id,
Kind, Manufacturer, Order number, Strip, Page, From, To, Cable core,
Cores) are for orientation and ignored on import. Editable: device and
cable Tag and Description, cable Length, terminal Name, Level, Order and
Jumper, wire Number, Color and Cross-section, and every
`properties.<key>` column. A `properties.<key>` column no row has yet
adds that property. Internal property namespaces are not exported.

## Matching

By `Id`. A deleted item's row is rejected ("no longer exists"); the
import never adds or deletes. Without an Id (CSV from elsewhere) devices
and cables match by a unique tag, terminals by strip and name; wires
cannot. Unknown columns and sheets are listed as notes and left alone.

## Preview and conflicts

`plan_import` returns an `ImportPlan` and changes nothing. Each `Change`
carries the item, column, the value now and the value in the sheet.
`.xlsx` files hold a hidden baseline (the exported values), so per cell:

| sheet vs baseline | project vs baseline | result |
| --- | --- | --- |
| same | any | sheet did not touch it; project's later edit stays |
| changed | same | plain change |
| changed | changed differently | **conflict**: unselected until resolved |

`ImportPlan::select` and `resolve` settle them. CSV has no baseline; the
plan says so. A row with any unreadable cell is rejected whole
(`Rejected` lists sheet, row, item and every reason), so a row is never
half-applied. Warnings cover tags two devices would share.

## Apply

`ImportPlan::apply(&mut Document)` runs one `Command::Batch` of record
replacements: one undo step. A plan made against an older project
revision refuses to apply (`ApplyError::Outdated`); preview again.

## Later

Cable cores sheet, new rows (add devices), terminal Kind, rejected rows
written back as a fix-and-reimport workbook, `kopercad-cli` and Python
entry points, app dialog (open file, preview table, conflict choice).
