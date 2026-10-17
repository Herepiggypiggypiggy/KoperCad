# rel-contactor roadmap

Scope: release probe finding A (contactor search offers only the coil; wires pass the coil).

- Done: Q search "contactor" now offers coil, 3-pole power contacts (first of the family), other poles, aux NO/NC. Multi-word queries ("power contactor") match.
- Done: test that power contacts dropped on a breaker-to-motor feed split all three wires onto contact pins (the wiring already worked; the probe placed the coil).
- Open: a coil dropped on one feed wire is inserted in series in that wire (legal, wrong for a power circuit). Possible check/warning, not done.
