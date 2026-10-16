# Commissioning report and tablet layout: handoff

Thread session: cse_015JWsgz1ouocSSfvtqPic6V. Branch: claude/project-thread-n01i61 (repo kopercad).
Sibling (data model, tick-off flow): cse_01Gpw3BVv45jMLL8rVjwtg4P. Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: `crates/novo-reports/src/commissioning.rs` has `CommissioningInput` (checks, punch items,
sign-offs), progress per cabinet and page, `readiness`, and `commissioning_report` giving a `ReportOutput`
(summary, cabinets, pages, checks, punch, signoff). CSV works through the existing `to_csv`.

Interface proposed to the sibling: they provide a mapping from their records to `CommissioningInput`.
Agreed by the sibling: they provide `to_report_input(&Project, &CommissioningStore)` in the new crate novo-commissioning (design PR #399); wait for their message that it landed, then register the report.

Resume: `cargo test -p novo-reports commissioning`. `novo_pdf::export_report_pdf` (crates/novo-pdf/src/report.rs) writes any ReportOutput as a stand-alone PDF.
Next: export command (menu, CLI), then the large-target tablet layout in novo-app.

## How to reach it (entry points)
- Palette / command field: "Export commissioning report" (`export.commissioning`), PDF next to the project file as `<project> commissioning.pdf` (our own earlier PDF is replaced, anything else stays).
- Commissioning tab (outline), "Report" row: PDF and CSV buttons, big touch targets.
- CLI: `kopercad-cli commissioning-report <project> [-o out.pdf|.csv|.xlsx]` (CSV on stdout without -o).
- Still open: File/Export menu item (UI integration thread, PR #452 export_menu), xlsx button, and the "Touch layout" toggle for the tablet layout (Workspaces thread).
Code: novo_reports::commissioning, novo_pdf::{export_report_pdf, report_bytes}, novo-app app/commissioning.rs, novo-cli lib.rs.

## Paused (usage limit, 2026-10-15 19:50 UTC): what is left
- PR #503 (entry points) is in review with Main reviewer C; if it is READY, do not push to it, follow-ups go in a new PR from main.
- File/Export menu item: add "Commissioning report..." to the Export submenu (export_menu in app/reports_hub.rs, #452) calling `export.commissioning`.
- Tablet layout: not started. Workspaces thread reserved View > Touch layout (docs/design/workspaces.md 5.3b): register `view.touch_layout` in novo-workflow COMMANDS with a Dutch word, a placement in menus.rs, setting in Settings, off by default, 32 px minimum rows. The Commissioning tab already uses 44 px buttons (TOUCH in app/commissioning.rs); the toggle should scale the whole tab and dock.
- Workspaces thread asked that the report also appear in the Reports workspace (#475) as a report id. It reads the sidecar store, not the Project, so it cannot be a plain `Report`; needs the hub to pass the store, or a Report built over a snapshot.
- xlsx button in the panel (engine supports it).
