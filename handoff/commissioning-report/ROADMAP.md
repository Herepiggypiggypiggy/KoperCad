# Commissioning report and tablet layout: roadmap

Scope: the commissioning report (CSV, Excel, PDF with sign-off block, progress per page and cabinet, punch
list) and a compact large-target layout for tablet or laptop use at the machine. The sibling thread
"Commissioning mode and punch list" owns the data model and the tick-off flow.

1. Report engine in `novo-reports::commissioning` over a plain `CommissioningInput` (PR 1). In progress.
2. Standalone report PDF in `novo-pdf` (A4 tables, header, footer with page n of m, sign-off lines).
3. Map the sibling's records to `CommissioningInput` once their types exist; register as a `Report`.
4. Export command (menu, CLI `kopercad-cli commissioning`), CSV and Excel.
5. Compact large-target layout in `novo-app` (touch-size rows, big pass/fail buttons), after DESIGN.md review.

Open: names of the sibling's data types and where they live.
