# Parts assignment and sample catalogue: handoff

Branch: `claude/project-thread-gqwjle`. PR #1581 (main merged in at 111a793 after #1572 landed).

## State
Engine, catalogue, dialog and Parts window fixes are written and tested. Review round 1 (R1, R2, R3, Main A, owner) is folded in: part facts cached per proposal call, narrower kind phrases first, motor efficiency and enclosure ambient labelled estimated, Siemens motor left as on main.

## Where things are
- Kinds and conflicts: `crates/novo-parts/src/fit.rs` (`Role`, `check_kind`, `conflict`).
- Proposals: `crates/novo-parts/src/assign.rs` (`Candidate`, `Confidence`, `PartProposal::default_choice`, `assign_choices`).
- Generic parts: `crates/novo-parts/catalog/generic.json`, loaded by `Catalog::starter()`. Edit the JSON by hand; keep `(generic)` in the description and "Not manufacturer data" in the licence note (a test checks both).
- Dialog: `crates/novo-app/src/app/parts_bulk.rs` (Edit > Parts > Assign parts to all devices, Ctrl+K "assign parts to all").
- Parts window: `crates/novo-app/src/app/parts.rs` (`toggle_parts_browser`, `device_selected`).
- Tests: `crates/novo-parts/tests/assign_kind.rs`, tests in `parts_bulk.rs`, QA `qa/scenarios/1581` to `1584` (assign dialog, long list, nothing fits, Parts window primary button) at both sizes.

## Traps
- Generic parts carry letter code K (PLC) or T (drive), so manual part suggestions for a `-K` device list PLC modules after the contactors; that is intended ("nothing is hidden").
- `Candidate` in novo-parts is the CAD-match type from #1572; the assignment type is `PartCandidate`.
- `part_role` / `PartFacts` read text; compute them once per part per batch, never per device and part.
- Never copy maker or customer data into `generic.json`.
