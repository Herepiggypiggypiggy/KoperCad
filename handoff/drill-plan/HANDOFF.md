# Drill plan handoff

Branch `claude/project-thread-yl68w2`, one PR from main. See ROADMAP.md.

Files: crates/novo-layout/src/drill_plan.rs (plan, Basis, pages), crates/novo-layout/src/model.rs (`drill_plan`), crates/novo-pdf/src/drill_plan.rs, crates/novo-app/src/app/plate_export.rs (`export_drill_plan_pdf`), workshop.rs (pack file), cabinet.rs (`Ask::ExportDrillPlan`, two menu spots), command `export.drill_plan` in novo-workflow commands/menus and novo-theme presentation.

Traps: hole numbers must stay equal to the drill table's; provenance never promoted (no key means "part data"); no screenshots in the repo (evidence goes to /mnt/project-files/qa/).
Resume: `CARGO_TARGET_DIR=<shared> cargo test -p novo-layout -p novo-pdf -p novo-workflow -p novo-theme`, then `-p novo-app` for the menu tests.
