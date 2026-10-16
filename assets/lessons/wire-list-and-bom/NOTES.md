# wire-list-and-bom: reviewer notes

Teaches: reading the wire list (wire 4 = `=A1+CAB-X1:1` to `=A1+FLD-M1:U1`, core BK of -W1) and the cable diagram in the Reports view, the bill of
materials (Qty per article, the section "Devices without parts"), giving a device a part and watching the BOM follow, and exporting the BOM to Excel.

Project: start = pump starter with the lamp -P1 wired in parallel with the coil and still without a part (own file). Finished = -P1 has part Siemens
3SU1106-6AA40-1AA0. documents.rs asserts the report rows; `lesson_verify_outputs.rs` drives the steps.

Verification: `crates/novo-app/src/app/lesson_verify_outputs.rs` (`wire_list_and_bom_every_step_works_from_its_own_start`, `..._solve_scripts_replay`).

Verified path per step
- open-wire-list / open-cable-diagram / open-bom: menu bar **Reports** > Wire list / Cable diagram / Bill of materials (commands.rs report.*, menus.rs;
  reports_view.rs show_report). Real clicks in the test; the command ticks the step.
- find-wire: wire 4 row asserted; **Filter rows (Ctrl+F)** typed "U1" keeps the U1 wires (asserted on the painted rows).
- open-cable-diagram: -W1, 4 of 4 cores, 0 spare (diagrams.rs; asserted).
- open-bom / see-missing: terminal block 3044076 Qty 3 used by -X1:1..3; section "Devices without parts" lists only -P1 (asserted).
- export-sheet: File > Export > **Export for bulk edit (.xlsx)** (export.sheets) writes "<project> sheets.xlsx" (sheets.rs).
- edit-part: Devices sheet columns Id, Tag, Description, Kind, Manufacturer, Order number (verified); the test edits the workbook as Excel would.
- import-part: File > Import spreadsheet (file dialog, not driven; `import_sheet_from` stands in), preview, "Apply 1 change (Enter)" applies; the lamp gets
  part Siemens 3SU1106-6AA40-1AA0 (a part the catalog does not know is created from the sheet, sheets/create.rs).
- bom-follows: one section, lamp row Qty 1 (asserted).
- export-bom: {key:view.command_palette} "export bom" or File > Export reports > Export bill of materials; writes "<project> - Bill of materials.xlsx" next to the
  project (reports_view.rs export_path), never over an earlier file; message "Exported <path>". Menu click verified.

Closed since the first version: export.bom now has a handler and a menu entry (real export taught); the Reports workspace with Excel and CSV buttons exists.

GAPS
- Assigning a part: still no control (inspector "Part" is read-only, Parts window only browses/imports; no command). Taught through the spreadsheet, the
  one working way. A picker replaces export-sheet, edit-part and import-part. Solve op wished: a spreadsheet import op (the `assign_part` solve stands in).
- The part 3SU1106-6AA40-1AA0 is not in the starter catalog, so the sheet creates a bare part (no data).
- Per-report export commands do not exist: only export.bom and export.reports (all reports, one sheet each); other lists use the Excel/CSV buttons of the Reports view.
- Anchors a pointer would use do not exist (`palette.field`, `outline.page.*`): no pointers on UI controls.

The `how` and `how_nl` lines are in lesson.toml.
