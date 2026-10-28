# Parts assignment and sample catalogue: roadmap

Source: laptop test report 2026-10-28 (cooling tower cell), findings 4, 20 and 23.

## Scope
- "Assign parts to all devices" must match by kind and rating, never by letter code alone.
- The shipped sample catalogue must cover a whole machine, labelled generic.
- The Parts window must close with Esc and Library > Parts, and show a primary action only when it can act.

## Done
- Kinds for drive, soft starter, power supply, PLC CPU and I/O, Ethernet switch, network port, fan, vent, enclosure, main switch, pilot lamp (`novo-parts/src/fit.rs`).
- A part of another kind, a coil for another voltage or another pole count is ruled out of a proposal (`fit::conflict`).
- Each candidate carries a confidence (Certain, Likely, Unsure) and a reason; only a clear choice is ticked (`PartProposal::default_choice`).
- The dialog lists every device with a tick, a part menu, the match and the reason (`novo-app/src/app/parts_bulk.rs`).
- `catalog/generic.json`: 40 generic parts (manufacturer `Generic`, typical values, estimated sources, authored provenance).
- Parts window: Library > Parts toggles, Esc clears a search then closes, "Use for selected device" is primary only with a device selected.

## Next
- Rebase on PR #1572 (Parts window, `parts.rs`, `lib.rs` exports) after it merges.
- Rated-power matching for drives is exact (`power_kw` equals `rated_power_kw`); an "at least" rule with a note is a possible refinement.
- Pole count is not checked for main switches (4-pole switch against 3-pole parts).
- The Quality tab still grades generic parts below Good (no 3D model, no price); that is honest, not a bug.

## Open questions
- None blocking.
