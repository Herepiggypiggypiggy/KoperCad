# Handoff: enclosure drill, BOM, reports

Code: `crates/novo-layout/src/spec_output.rs`; hook in `model::drill_plan` (sets `DrillPlan.enclosure`); `crates/novo-reports/src/bom.rs` `enclosure_sections`.
Tests: `novo-layout` unit tests and `tests/spec_output.rs`, `novo-reports/tests/enclosure_bom.rs`. Each changes the spec and checks the output changes.
Rules: no silent defaults (missing field is named), old projects without a spec are unchanged, `Estimate from catalogue data` notice while a value is catalogue/default.
Branch is based on E1 (#1891 head 6a0a7db); rebase onto main after it merges, then open the PR.
