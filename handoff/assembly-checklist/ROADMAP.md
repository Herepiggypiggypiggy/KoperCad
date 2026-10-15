# Assembly checklist roadmap

Scope: the R8 workshop checklist from docs/PRO-PANEL-GAP-ANALYSIS.md, in
novo-reports. Machine-neutral (table, CSV, PDF). Design: docs/design/assembly-checklist.md.

Done
- AssemblyChecklist report with five steps per cabinet and 12 tests.

Next, in order
1. Drill step from `novo_layout::model::plate_documents` / `DrillTable` once #397 merges.
2. PDF through the stand-alone report PDF once #404 merges (row_mm for sign-off rows).
3. Menu entry and CLI export, through the app owners.

Open questions
- Which machine, if any, needs its own file. Not blocking.
