# Migration and interop: roadmap

Scope: gap #10 of `docs/AMBITION-REVIEW.md` (PR #504). Design: `docs/design/interop-migration.md`.

Done (first PR, branch `claude/project-thread-6oep4z`):
- `novo_interop::report` (what moved, what was left out)
- `step`: cabinet as STEP assembly (AP214), checked with OpenCASCADE
- `aml`: AutomationML export, tolerant reader, preview and one-undo-step import
- `dwg`: free converter driver (LibreDWG, ODA File Converter), separate process
- File > Import and export, command field, `kopercad-cli export step|aml`, `convert-dwg`

Next, in order:
1. Standard import: title blocks/frames from DXF, symbols from DXF/SVG into a user pack, macros
2. Project-level migration: foreign Excel/CSV mapping, nets to generated pages with wires
3. PLCopen XML and PLC hardware config (with the PLC lane)
4. STEP depth: wire tubes, maker geometry, door angle, real wall thickness
5. Python bulk API (WS-28)

Open questions:
- Real enclosure wall thickness and depth belong in the catalogue data (part lane).
- The user's standard folder (`C:\Users\tvelzel\Documents\Standaard eplan Coen - kopie voor v2022`) is a device folder the thread may read through Remote Control; fixtures from it stay off the repo.
