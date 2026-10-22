# Wire production data: handoff
Branch claude/project-thread-2fmuyy (kopercad). Engine: crates/novo-reports/src/wirecut.rs (assign_tracks, WireEnd.processing, END_PROCESSING_KEY wire property). Consumers: bench.rs, packet.rs, app bench cut table. QA: qa/scenarios/71-wire-production-bundles.qa, shots in /mnt/project-files/qa/wire-production-data/.
Decisions: bundle = identical duct sequence (either way round), numbered by track; sequence = longest first; unrouted wires have no bundle and say so. Basis labels: stated / part / typical / unknown.
Trap: packet cover is full; the test every_cover_note_stays_on_the_cover fails if a note is added.
