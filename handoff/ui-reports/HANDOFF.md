# UI integration: reports and checks: handoff

Branch `claude/project-thread-8ngiwg` (repo kopercad). Code: `crates/novo-app/src/app/reports_hub.rs`
(`Hub`, `hub_show`, `export_reports_to`, `export_menu`), tests `reports_hub_tests.rs`; hooks in
`app.rs` (`hub.open` beside `cabinet_open`, top-bar segment), `actions.rs` (`ShowReports`, Ctrl+6),
`workbench.rs` (`export.reports`, `export.bom`), novo-workflow (`view.reports`, `export.reports`, Dutch words),
`settings.rs` (`csv_semicolon`).

- The hub reads `self.wb.reports.live(project)`; any report registered in `standard_reports()` shows up.
- Exports ask for the file with a save dialog (never overwrite a user's spreadsheet silently);
  `export_reports_to` is the testable core.
- Resume: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-app -p novo-workflow`.
- Not run in the real window: checked only headless. Visual check is Tadjiro's or a build.
- The Workspaces thread may fold the `hub.open`/`cabinet_open` flags into a workspace enum; keep the hub
  self-contained so that is a small change.
