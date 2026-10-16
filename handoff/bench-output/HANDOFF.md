# Bench output: handoff

State 2026-10-16: first PR from branch `bench-output` (see PROGRESS.md for the head SHA and review state).

Code:
- `crates/novo-reports/src/bench.rs`: `FerruleStandard`, `FerruleTable` (id `ferrules`), `BenchCut` (id `benchcut`), `ferrule_counts`.
- `crates/novo-reports/src/label_sheet.rs`: `LabelStock`, `LabelSettings`, `label_items`, `layout_labels`, `stock_presets`.
- `crates/novo-reports/src/wirecut.rs`: `wire_cut_jobs_with(project, &standard)`; the strip length of a wire end without a part value is the standard's contact length (defaults equal `typical_strip_mm`, so existing output is unchanged).
- `crates/novo-pdf/src/report.rs`: the ballot box character (`TICK_BOX`) is drawn as a square.
- `crates/novo-app/src/app/bench.rs`: Bench output window (Cut list, Ferrules, Labels tabs) working on drafts; exports use the drafts; "Save to project" is one undo step.
- Tests: `crates/novo-reports/tests/bench.rs`, `crates/novo-app/src/app/bench_tests.rs`.

Decisions:
- Ferrule colours: DIN 46228-4 as commonly sold; above 16 mm² makers differ. It is a default to edit, not a claim of compliance.
- Label preset pitches come from published sheet data from memory and were never printed here: the PDF is 1:1, print at 100%, use the test outlines and the shift fields.
- Lengths are exact only for wires routed through ducts; everything else prints `~`.
- Twin ferrules where two wire ends of one size share a pin, up to 6 mm² by default.
- The older `cutlist` report (wire list grouped) and `wirecut` (machine CSV, workbook) are unchanged; the bench PDF is generated from the same wire jobs as `wirecut`.

Resume: `cargo test -p novo-reports --test bench`, `cargo test -p novo-app bench`.
Reviewers: R5 for reports/UI (cse_01DHLANQPyc634wMAeUYHDHz), R2 for data (cse_01SEokxUZ1MKDwyQvLm4DPys).
