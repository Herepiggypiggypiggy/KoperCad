# UI integration: reports and checks: roadmap

Scope (asked by Tadjiro 2026-10-15): the output features other threads build must be
usable in the app, not only in engines and the CLI. This thread owns where reports and
checks live in the UI, the Reports hub, the Issues panel placement and polish, export
buttons and menu entries. Engines stay with their owners (novo-reports, novo-checks,
novo-pdf, novo-revisions, commissioning). Coordinates with Workspaces, menus and
navigation (cse_01WAQSEGkMbyreCPG18tsSPc) on view switcher and menus, and with Design
checks panel (cse_0191A95sdns9gs3sLWgmXiis, PR #413) on the Issues list.

1. Reports hub (Ctrl+6): third view next to Schematic and Cabinet; every registered
   report as a live table, filter, section tabs, row click goes to the drawing, Excel
   (this report, or all as one workbook), CSV (comma or semicolon), Show page. PR 1.
2. Export submenu in the project menu, `export.reports` and `export.bom` commands. PR 1.
3. Issues panel polish on top of #413: group by kind or page, severity chips with counts,
   waive flow reachable from the row (button, not only right-click), Design checks as a
   report in the hub with CSV/Excel/PDF. After #413 merges.
4. Revision change list: rows from `novo_revisions::change_list` (#401) in the Changes
   outline and as a report in the hub. After #401 merges (revision thread owns the app part).
5. Commissioning report: Export entry once #404 and novo-commissioning land.
6. Per-report PDF (`novo_pdf::export_report_pdf`, #404) as a button in the hub.
7. Labels (terminal and wire labels) once an engine exists.

See PROGRESS.md for the feature versus location table.
