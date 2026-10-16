# wire-list-and-bom: reviewer notes

Teaches: reading the wire list (wire 4 = -X1:1 to -M1:U1, core BK of -W1), the cable diagram as cable schedule (overview sheet 50a),
the bill of materials (Qty per article, the "Devices without parts" sheet), giving a device a part and watching the BOM follow
(10a + 10b become one sheet 10), and exporting the BOM.

Project: start = pump starter with the lamp -P1 wired in parallel with the coil and still without a part (own file, not the finished
state of lesson 1, because lesson 1 leaves the lamp unwired). Finished = -P1 has part Siemens 3SU1106-6AA40-1AA0 (BOM row 10, Qty 1).
documents.rs asserts the wire list rows, the cable row, both BOM states, sheet names and the row positions used by the pointers.

Verified UI path per step
- open-wire-list / open-cable-diagram: PageDown keymap.rs:385; outline rows backend.rs:729.
- find-wire, read-cable-schedule, read-parts: report columns wires.rs:189-203, diagrams.rs:142-153, bom.rs:102-115.
- see-missing: BOM section `without_parts` bom.rs:137; the Issues list shows the same device ("has no part", info).
- bom-follows: reports.rs:45-52 regenerate on change sets.

GAPS (lesson stays draft)
- assign-part: no control assigns a part to a device. Inspector "Part" is a read-only label (inspector.rs:527), Devices drawer is read-only
  (drawers.rs:345), Parts window only browses/imports (parts.rs). Engine: novo_parts::assign_part (assign.rs:85). Also needs a solve op.
- export-bom: `export.bom` is a listed command (commands.rs:722) but has no handler: the app says "Export bill of materials is not in the
  app yet" (workbench.rs:553). Engine: novo_reports::save_xlsx / to_csv; command line `kopercad-cli export bom|wires|devices|reports`.
- The step texts name the Ctrl+K path because export.bom has no key.
