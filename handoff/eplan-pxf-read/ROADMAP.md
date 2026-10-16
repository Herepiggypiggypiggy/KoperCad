# EPLAN-PXF-READ roadmap
Scope: read EPLAN PXF (.epj project, .emp macro) into KoperCAD, counted and undoable. Design: docs/design/eplan-pxf.md.
- [x] Slice 1: streaming reader, model, report, synthetic fixtures (#607 merged) + linear tag scan follow-up
- [ ] Slice 2: PxfPlan preview + apply through commands (one undo step)
- [ ] Slice 3: File > Import > EPLAN project (PXF), preview dialog, kopercad-cli import-eplan
- [ ] Slice 4: interruption points to xrefs, .emp into macro library
- [ ] Slice 5: nets (with EPLAN-NETS), symbol mapping (EPLAN-SYMBOLS)
Open: a real non-customer .epj from Tadjiro would confirm the field table (findings doc section 6).
