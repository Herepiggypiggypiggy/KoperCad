# Spreadsheet import, app and CLI: handoff

Branch claude/project-thread-bpubxq (kopercad), PR based on #414's branch (claude/project-thread-k6t1zj) until #414 merges; then retarget to main.

- App: crates/novo-app/src/app/sheet_import.rs. State `wb.sheet_import`; modal; `import_sheet_from(path)` is the test hook. Applies `ImportPlan::command` through `NovoApp::execute` so the canvas and storage update (not `plan.apply`).
- CLI: `import_sheet` in crates/novo-cli/src/lib.rs. The CLI never writes over a project: `--output` must be a new `.novo` path. Exit 1 when rows were rejected.
- Row tick = all non-conflict changes of one sheet row; conflict cells have their own tick (start unticked).
- Traps: the engine's plan refuses to apply if the project revision moved; the dialog is modal so it cannot. `import.spreadsheet` also added to novo-workflow's command list.
- Talk to: engine threads cse_01ToRihU88DzNsNUPssCav6H (#414), cse_011McCS481zaaDZqTGbrh5KN (#411).

## How to reach it
- App: project menu (the KoperCAD mark) > **Import spreadsheet…**, or the command palette: command id `import.spreadsheet` (verbs "import spreadsheet", "import sheet", "import excel"). Needs an open project. Export is the round-trip thread's menu command, not this one.
- CLI: `kopercad-cli import sheet <project.novo> <file.xlsx|csv> [--json] [--output new.novo] [--resolve sheet|project] [--skip Devices:5]`.
- Dialog shows: changed rows, new devices (`plan.new_devices`, own tick, "New" filter), conflicts, rejected rows with line and reason, warnings next to Apply, notes. Not yet: "Save rejected rows" (needs #430's `rejected_workbook`).
