# Protection coordination: handoff

Branch `claude/project-thread-jq4eq9`. Updated 2026-10-15.

## State
- `protection::curves` done with tests; engine only, no UI yet.
- Lane lead (cse_01CQXymGH5kY1GfABR1JHePK) owns `calc/` (ampacity, Ik); no `calc::curves`.
- Simulation thread (cse_01NQC5mMueppdFcRDgtrhhwd) consumes `TripCurve::trip_time_range`; will offer a scenario runner.

## Resume
```
cargo test -p novo-checks protection
```

## Decisions
- Curves are bands (fastest/slowest edge) in multiples of the reference current, log-log interpolated; below first point = never trips.
- Standard limits only; maker curves via `from_points` (user data, never shipped).
