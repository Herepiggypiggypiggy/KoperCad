# Route all through ducts: progress

- 2026-10-28: reproduced on the example (wires started at device middles and crossed bodies to the left trunk). Fixed as above, tests and QA scenario added.
- 2026-10-28: follow-up branch from main: slots, device bodies, DIN rail channels built and tested in the engine; UI and Issues wired.
- 2026-10-29: follow-ups from the #1649 review: tie-break test and renamed rail-vs-duct test, release-note lines (RELEASE_NOTES.md), rail channels vs neighbouring rows, slot snap past the last slot.
- 2026-10-29: route-all speed: with device bodies the stub round them is worked out for the 8 nearest ducts and 4 nearest rail channels per terminal (rest only if all blocked), own and nearby bodies found once per terminal. 1500 wires on 360 devices: 3.2 s to 0.5 s (debug test build), `crates/novo-routing/tests/speed.rs`.
