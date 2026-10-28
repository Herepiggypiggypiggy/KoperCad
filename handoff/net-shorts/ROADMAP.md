# Net shorts roadmap

Scope: design checks for conductors that meet where they must not, found by the cooling tower laptop report (findings 3, 19, 25 and the checks half of 15). Pure checks in `novo-checks`, no UI of their own: findings appear in Issues (the status pill) like every check.

## Done (PR 1)
- `novo-checks/src/net_shorts.rs`: a `PE` or `N` pin on a net with a phase or DC positive; two poles of one breaker, switch or contactor joined on one side while the other sides carry different permanent conductors. Reported as `PotentialConflict` (error) with the pages.
- "No duct serves this wire" is one problem per page that names the wires by their ends.
- A symbol without a tag is named in words, never by library id.
- A name used by more than two interruption points is an info (a branch), not a warning.
- A lone interruption point names the other lone points and says how to pair them (Link cross reference).
- Test: `crates/novo-tests/tests/short_checks.rs`; fixture `qa/fixtures/cooling-tower-shorts.novo`; QA scenario `qa/scenarios/1600-cooling-tower-shorts.qa`.

## Next
- Two phases that meet through separate single-pole devices are not found (that wiring is how a changeover is drawn). A live-simulation check could settle it.
- Offer a fix: select the wires of the net, or show the bridging wire. Today the message marks the device and its wires on the net.
- Name the wires of the other "free air" style groups the same way (`GuessedWire` lists no wires).

## Open questions
- Is N joined to PE downstream of the split worth a warning? Left out: TN-S allows the bond at the source and the drawing cannot tell where the source is.
