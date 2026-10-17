# REL-BENCH: handoff

Branch `claude/project-thread-fr80pa`; one PR per slice from main (see PROGRESS.md for the PR list).

State: slices written; see PROGRESS.md for which are merged.

Slices
1. Export toasts: STEP/AML/EPLAN, PLC tags and the safety report raise a file toast with "Open folder"; `projects_dir()` falls back to the home folder when Documents is unknown.
2. Fill in defaults: `novo-reports/src/wire_defaults.rs` (`unset_wires`, `apply_wire_defaults`) and the "Fill in" row in `novo-app/src/app/bench.rs` (Reports > Bench output > Cut list). Only unset wires change; earth gets GNYE, neutral and 0 V BU, the rest the typed colour; one undo step.
3. Regression tests: `novo-tests/tests/bench_numbers.rs` (no number shared across the overload relay; feed and PE wires on the cut list).

Traps
- Cut list colour is the wire's own `color`, not the potential colour the 3D view shows; the fill row writes real values so the PDF agrees.
- The wire numbering fix is in `group.rs`; do not number by net.
Contacts: coordinator `session_01Na1FZm8gtMxPVXh4qycirm`; cut list thread `cse_01BmmFqnrd6V11gM5thKEwKH`.

Resume: `cargo test -p novo-reports -p novo-numbering -p novo-tests`, then `cargo test -p novo-app bench`.
