# Project queries and "My reports"

A read-only query layer over the project, and saved reports built on it.
Our own implementation; the idea (a flat, queryable view of the project that
reports read) is common to many tools, the code is not shared with any.

## Where to find it

- **Reports workspace > My reports**: the editor (Edit my reports), the
  saved reports as tables, and Export (PDF, Excel, CSV) and Add as page like
  any report. Rows jump to the drawing like other reports.
- **Command line**: `kopercad-cli query project.novo "SELECT ..."`,
  `--preset NAME`, `--presets`, `--tables`, `--json`, `-o file.csv|xlsx`.
  Exit 2 for a query that is wrong.
- **Rust**: `novo_reports::query` (`run`, `check`, `schema`, `Presets`).

## Tables

`pages`, `devices` (the device list), `terminals`, `wires` (the wire list
including supply and PE leads), `cables`, `cores`, `parts`. Each also gets a
`prop_<name>` column per property key found on its rows. Devices and wires
come from the same code as the reports of that name, so a query and the
report agree. `kopercad-cli query FILE --tables` lists every column.

## Language

`SELECT [DISTINCT] items FROM table [WHERE] [GROUP BY] [HAVING] [ORDER BY] [LIMIT]`.
Comparisons, `AND/OR/NOT`, `LIKE`, `IN`, `BETWEEN`, `IS [NOT] NULL`, `+ - * / ||`.
Functions: `lower upper trim length abs round coalesce ifnull substr replace concat`.
Totals: `count sum avg min max group_concat` (`count(*)`, `count(DISTINCT x)`).
Text compares without regard to case and "naturally" (K2 before K10); an empty
cell is `NULL`. No joins, no subqueries, and nothing but `SELECT`: a query never
changes the project. Errors carry the position and suggest the closest name.

## Saved reports

Stored as add-on block `reports/presets` (JSON, format 1) in the project file:
they travel with the project, are part of undo, and an older program keeps them
untouched. The report `queries` ("My reports") prints one table per saved report;
a report that no longer runs prints its problem instead of disappearing.

## Not built (honest list)

Joins between tables, user-wide presets shared between projects, a report
template per preset, reading `.novo` files from Python through the same layer
(the CLI covers scripts for now).
