# Plate drawing: roadmap

Scope: R5 of docs/PRO-PANEL-GAP-ANALYSIS.md. Dimensioned mounting-plate drawing
plus drill-hole and cut-out table, PDF (novo-pdf) and DXF (novo-interop).

Done: engine (novo-layout drill, plate_drawing, model::plate_documents), PDF
writer, DXF writer, tests, docs/PLATE-DRAWING.md.

Next, in order:
1. Entry points. Done in #470: palette command `export.plate` and `kopercad-cli export plate`. Still open: Cabinet header action, File > Export item, DXF/dialog (call `NovoApp::export_plate_pdf()`; originally requested from requested from the UI integration threads cse/session_01G52sVY5N34jPsrkqWukApR and session_01WAQSEGkMbyreCPG18tsSPc): Cabinet header action Plate drawing (PDF, DXF), File > Export items, palette commands, `kopercad-cli export plate`. Engine calls are in docs/PLATE-DRAWING.md.
2. Duct cut-outs and duct mounting holes; side view (R2 adds enclosure depth and door).
3. DONE in #477 (draft, stacked on #409): real door size from novo_layout::model::enclosure_of (#409: door = enclosure width x height, plate centred), fallback plate-sized; follow-up PR after #409 merges, not pushed to #397.
4. Machine formats, only when the workshop names its machine.

Open: part keys are text for now (R0 may add typed fields); keep `hole_pattern` and `cutout` readable if so.
