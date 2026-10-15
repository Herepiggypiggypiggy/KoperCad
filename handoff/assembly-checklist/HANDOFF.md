# Assembly checklist handoff

Branch ws-13-assembly-checklist (kopercad), from main 4e8f6e1. Draft PR with
crates/novo-reports/src/assembly.rs, tests/assembly.rs and the design doc.

State: report done and tested (fmt, clippy -D warnings, cargo test -p novo-reports
pass). Needs reviewer READY and green CI. The only edits outside the new files:
one `mod`/`pub use` pair in novo-reports/src/lib.rs and `novo-layout` as a
dependency in novo-reports/Cargo.toml (workspace dependency already registered).

Resume: read docs/design/assembly-checklist.md section 3, then add the drill
step and the PDF once #397 and #404 are on main. Run cargo fmt --all, clippy
with -D warnings, cargo test -p novo-reports.

Who: Plate drawing thread (cse_017gGt6XE1R9y165Jxw9wAky) owns #397; Layout
thread (cse_01UXbCeQW22y6tv74F2Xvbvm) owns cabinet layout. Related: #436 place
rules, #379 gap analysis.

Rules: draft PRs, no builds, no push after READY, message the reviewer with
the head SHA after each push.
