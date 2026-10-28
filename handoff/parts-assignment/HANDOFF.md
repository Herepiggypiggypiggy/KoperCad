# Parts assignment and sample catalogue: handoff

Branch: `claude/parts-assign-kind` (from main 78a1dab). PR: see PROGRESS.md.

## State
Engine, catalogue, dialog and Parts window fixes are written and tested. novo-parts and novo-app tests for these pass; clippy is clean.

## Where things are
- Kinds and conflicts: `crates/novo-parts/src/fit.rs` (`Role`, `check_kind`, `conflict`).
- Proposals: `crates/novo-parts/src/assign.rs` (`Candidate`, `Confidence`, `PartProposal::default_choice`, `assign_choices`).
- Generic parts: `crates/novo-parts/catalog/generic.json`, loaded by `Catalog::starter()`. Edit the JSON by hand; keep `(generic)` in the description and "Not manufacturer data" in the licence note (a test checks both).
- Dialog: `crates/novo-app/src/app/parts_bulk.rs` (Edit > Parts > Assign parts to all devices, Ctrl+K "assign parts to all").
- Parts window: `crates/novo-app/src/app/parts.rs` (`toggle_parts_browser`, `device_selected`).
- Tests: `crates/novo-parts/tests/assign_kind.rs`, tests in `parts_bulk.rs`, QA `qa/scenarios/1581-assign-all-never-matches-wrong-kind.qa`.

## Traps
- Generic parts carry letter code K (PLC) or T (drive), so manual part suggestions for a `-K` device list PLC modules after the contactors; that is intended ("nothing is hidden").
- PR #1572 also edits `parts.rs` and the `lib.rs` exports; expect a small rebase.
- Never copy maker or customer data into `generic.json`.
