# Protection coordination: handoff

Branch `claude/project-thread-jq4eq9`. Updated 2026-10-15.

## State
- Engine in `crates/novo-checks/src/protection/` (curves, devices, selectivity, coordination, sccr); all pure data and functions, 60 unit tests, 3 integration tests on the Gate 1 starter, doctests; clippy and fmt clean.
- Model reader `protection::model` and the Protection window (`crates/novo-app/src/app/protection.rs`, command `view.protection`, test `protection_tests.rs`) are in the same PR.
- Lane lead (cse_01CQXymGH5kY1GfABR1JHePK) owns `calc/` (ampacity, Ik); there is no `calc::curves`.
- Simulation thread (cse_01NQC5mMueppdFcRDgtrhhwd) consumes `TripCurve::trip_time_range` and offers a scenario runner.

## Resume
```
cargo test -p novo-checks protection
```

## Decisions
- Curves are bands (fastest/slowest edge) in multiples of the reference current, log-log interpolated; below the first point = never trips.
- Standard limits only; maker curves via `from_points` / `LetThrough` / `MakerTables` (user data, never shipped).
- Without let-through data a breaker's instantaneous region is reported "unchecked" (Info), never as a failure.
- Selectivity result lists non-selective ranges, maker table overrides computed value, computed limit still reported.
- SCCR transformer secondary uses the infinite-source figure, converted to a line-side rating with the source impedance.

## Traps
- Do not add `ProblemKind` variants without the WS-14 owner; findings carry stable `Rule` keys (`protection.*`).
- `pub mod protection;` in lib.rs and serde deps in Cargo.toml may conflict trivially with #412 (`pub mod calc;`).
