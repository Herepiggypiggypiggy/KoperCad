# Assembly checklist roadmap

Scope: the R8 workshop checklist from docs/PRO-PANEL-GAP-ANALYSIS.md, in
novo-reports. Machine-neutral (table, CSV, PDF). Design: docs/design/assembly-checklist.md.

Done
- File > Export assembly checklist (PDF) and palette command `export.assembly` (novo-app `assembly.rs`).
- AssemblyChecklist report with six steps per cabinet (prepare, drill, mount, wire, label, check) and 14 tests; drill step from #397's plate_documents.

Next, in order
1. Per-row heights for the sign-off lines only (today every section has 8 mm rows).
2. Print the plate drawing beside the drill step (novo-pdf plate drawing).

Open questions
- Which machine, if any, needs its own file. Not blocking.
