# T12 Import, export, reports, files: roadmap

Scope (LANE-PROMPTS.md): failed import/export/save/PDF/DXF/EPLAN/bench/workpack/handover/report jobs become Job flags with Retry, Open folder, Copy details; keep the "written file" toast. 143 rows in `sites/files.md`. Scenario block 9520-9529.

| PR | Content | State |
|---|---|---|
| 1 | Job-flag mechanism (`app/flags_jobs.rs`), top bar job triangle, popover actions, every failed-job toast (class J, Level::Error/Warning via `message`) in the files area, census file | this PR |
| 2 | Class P rows on the bench, ferrule and label tabs (banners and toasts): triangle on the field or tab | next |
| 3 | Class P/R rows in print preview, plot sets, PDF options, reports view, finish_export caveats | next |
| 4 | Class P rows in variants, revisions, handover, workpack, sheet import dialog, EPLAN import dialog | next |
