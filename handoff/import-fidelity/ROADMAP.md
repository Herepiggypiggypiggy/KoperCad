# Import fidelity: roadmap

Scope: make the EPLAN project (.epj/.pxf), crane configurator and AutomationML imports faithful, and say honestly what is exact, guessed or dropped.

Done
- #1069: symbols by pin count (AML kinds, crane boxes 60 -> 14), part sizes from the sheet outline and AML Width/Height/Depth, pole tags once, short interruption names.
- Step 1: the import report has a `guessed` section next to `lossy` (boxes, loosely matched symbols, wire routes, device footprints, rails and ducts, devices without a footprint). Shown in the AML and EPLAN import dialogs, `kopercad-cli import ... --json` ("guessed") and the text report.

- AML export carries pages, placed symbols (position, pins, faces, KoperCAD library key) and links the nets to the drawn pins; reading it back gives the same artwork and nets (round-trip test). Still lost in AML: wire routes, page texts, lines, frames.

Next, in order
1. More AML and EPLAN cases from real projects (kinds that still fall back to a box; round-trip checks: write AML, read it back, compare).
2. Text anchors (waits on an EPLAN preview screenshot from the owner).
3. Parts import of `interop/private/crane/part-sizes.csv` verified in the cabinet view.

Open questions: none.
