# rel-contactor: overload search

PR from main. Q search: a word that names a device ranks it above that device's auxiliary contacts (-150 for aux contact symbols without a typed alias; power contacts exempt), `thermal` and `motor protection` keywords on the 3-pole overload relay, favourites win ties (+20; same hunk as the contactor search PR #963).
`motor protection` still lists the motor protection breaker first (it carries the name); the relay is second.
Open: the aux contact auto-wiring onto pole 1 of a three-pole feed. The coil warning (PR #972) covers coils only; extend `coil_on_power_feed` to aux contacts after #972 merges.
