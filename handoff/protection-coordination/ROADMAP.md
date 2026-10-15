# Protection coordination: roadmap

Thread "Protection coordination and selectivity" (Lane B of "Protection and cable engineering"). Code: `crates/novo-checks/src/protection/`.

## Scope
Trip curves (shared model, owner: this thread), selectivity and back-up analysis, SCCR (UL 508A) engine, maker trip data as user-supplied data, UI to see curves and results.

## Done
- curves: Band/TripCurve, MCB B/C/D/K/Z, gG fuse, overload relay classes, MPCB, LSI electronic trip, maker points.

## Next
1. selectivity: curve overlay limit current, fuse I²t, zone (time) selectivity, maker tables, back-up
2. sccr: component tree, default ratings, current-limiting upgrade, panel rating
3. adapter from model (part properties) and app view (curves plot, selectivity table)
4. shared scenario runner with Simulation thread

## Open questions
- Part property keys for curve data (agree with lane lead).
- Numbers: standard limits are from memory of IEC 60898-1 / 60269 / 60947-4-1; verify against the editions in force.
