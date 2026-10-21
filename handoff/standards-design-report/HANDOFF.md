# Standards compliance report, design verification: handoff

Branch `claude/project-thread-6tx8ln` (kopercad). Builds on the merged standards report.

Where the user finds it: File > Export reports > Export standards compliance report (palette: "Export standards compliance report"), or Check > Standards check, Save report as PDF / web page. The report now always has a "Read this first" box and a Design checks section, even with no standard selected.

Code: `crates/novo-checks/src/standards/design.rs` (families, rows, HeatFact), `document.rs` (`build_with`, `LIMITS`, HTML), `crates/novo-app/src/app/standards_report.rs` (PDF table mapping `design_sections`, `heat_fact`), tests in `standards_report_tests.rs`, QA `qa/scenarios/57-standards-report.qa`.

Honesty rules baked in: IEC 60204-1 clause numbers unverified; ampacity, colour and motor tables are planning values; PFHd for cat 2 to 4 is an estimate; heat is a model. A family with nothing to examine is "Not applicable", missing data is "Partly/Not checkable", never a pass.

Traps: `DesignInput::from_project` leaves out house-rule wire problems (only novo-reports reads those); the app passes `novo_reports::issues::open/waived`. Heat staleness uses `heat.live.is_stale()`, which lags until the 3D view has drawn once after an edit.
