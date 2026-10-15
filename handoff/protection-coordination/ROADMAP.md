# Protection coordination: roadmap

Thread "Protection coordination and selectivity" (Lane B of "Protection and cable engineering"). Code: `crates/novo-checks/src/protection/`.

## Scope
Trip curves (shared model, owner: this thread), selectivity and back-up analysis, SCCR (UL 508A) engine, maker trip data as user-supplied data, UI to see curves and results.

## Done
- curves: Band/TripCurve, MCB B/C/D/K/Z, gG fuse, overload relay classes, MPCB, LSI electronic trip, maker points.
- devices: Device, LetThrough, Withstand, MakerTables (JSON).
- selectivity: curve overlay, energy discrimination, maker table override, Icw cap, non-selective ranges; back-up (table or let-through).
- coordination: chain checks (breaking capacity, back-up, selectivity, disconnection time, conductor withstand), motor start.
- sccr: component tree, defaults, combinations, current-limiting upgrade, transformers, marking text.

## Next
1. Adapter: model device -> `Device`/`Node` (part properties; agree keys with lane lead after #454), and `Problem` kinds (ask panel/WS-14 owner)
2. App: protection view (log-log curve plot, selectivity table, SCCR panel, maker table import)
3. Shared scenario runner with Simulation thread (per-device trip, clearing device, non-selective, backup flags)
4. More curve families: UL 489 breakers, class RK/J/CC fuses, aM motor fuses, transformer inrush and damage curves, cable damage curve overlay
5. Warm-start relays, ambient derating of breakers

## Open questions
- Part property keys for curve data (agree with lane lead).
- Numbers: standard limits are from memory of IEC 60898-1 / 60269 / 60947-4-1 / UL 508A SB4.1; verify against the editions in force. gG I2t is an estimate (0.7*In^2.5).
