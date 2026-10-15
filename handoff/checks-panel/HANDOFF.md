# Design checks panel: handoff

Branch `claude/project-thread-h42ie7`, draft PR #413.

## Resume
```
git fetch origin claude/project-thread-h42ie7
cargo test -p novo-checks -p novo-pdf
```
Heavy test binaries (novo-app, gate1) can fill the cloud disk: run `cargo clean -p novo-app` first and use `CARGO_INCREMENTAL=0`.

## Decisions
- One issue type: novo_checks::Problem. Waiver = Dismissal `check.<ProblemKind::key()>` on the sorted objects in `at`, reason in property `reason`; goes away with any object.
- Calc logic belongs to the calc thread (novo-checks::calc, #412); property keys they read: length_m, load_current_a, breaking_capacity_ka, rated_current_a, trip_curve, supply.short_circuit_ka, supply.voltage_v, check.voltage_drop_pct.
- The PDF prints design checks pages only when there is something to report.
