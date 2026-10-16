# Assembly checklist handoff

Branch ws-13-assembly-checklist (kopercad), main merged after #397. Draft PR with
crates/novo-reports/src/assembly.rs, tests/assembly.rs and the design doc.

State: report (six steps incl. drill) done and tested (fmt, clippy -D warnings, cargo test -p novo-reports
pass). Needs reviewer READY and green CI. The only edits outside the new files:
one `mod`/`pub use` pair in novo-reports/src/lib.rs, and `novo-layout` as a
dependency in novo-reports/Cargo.toml (workspace dependency already registered).

Resume: read docs/design/assembly-checklist.md section 3, then add the PDF once
#404 is on main (#397 is merged; the drill step is in). Run cargo fmt --all, clippy
with -D warnings, cargo test -p novo-reports.

Who: Plate drawing thread (session_01M3YSp4hsTrqSY24mQ1FWW1) owns the plate drawing engine; Layout
thread (cse_01UXbCeQW22y6tv74F2Xvbvm) owns cabinet layout. Related: #436 place
rules, #379 gap analysis.

Rules: draft PRs, no builds, no push after READY, message the reviewer with
the head SHA after each push.

Update 19:45 UTC: #451 merged. The in-app entry is on branch ws-01-assembly-export (File menu + export.assembly); see docs/design/assembly-checklist.md section 3.
