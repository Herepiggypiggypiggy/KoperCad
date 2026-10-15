# Place rules (R7): handoff

Branch `ws-15-place-rules`, draft PR "Layout: rules-based Place all (R7)". Engine only: new `crates/novo-layout/src/rules.rs` (one line in `lib.rs`), tests in `crates/novo-layout/tests/rules.rs`, doc section 5b.

Decisions: a rule set is plain text (like keymap.txt); the planner returns the existing `plan::Plan`; no change to `plan.rs`, `state.rs`, `model.rs` or `novo-app` to avoid conflicts with #409, #419, #406. Part clearances are read by key name (`clearance_*_mm`, `power_loss_w`, `load_factor`), the same keys as #419 and #406.

Resume: `cargo test -p novo-layout`. App wiring comes next, see ROADMAP.

Update 19:55 UTC: #436 merged. The app wiring is on branch ws-15-place-rules-app, stacked on #409 (it edits the Plate card that #409 gives the Inside and door section).
