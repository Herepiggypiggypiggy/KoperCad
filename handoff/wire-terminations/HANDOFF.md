# Handoff: wire terminations

Branch `claude/project-thread-mp6n5q`. Code: `crates/novo-reports/src/termination.rs` (kinds, parse, `crimp_list`), `wirecut.rs` (`WireEnd.termination`, `WireCutJob.sleeve`, JOB_COLUMNS now 26), `labels.rs`, `packet.rs`, `bench.rs` (lugs section), app: `wire_panel.rs` (Inspector), `bench.rs`/`bench_packet.rs`.
Tests: `crates/novo-reports/tests/termination.rs`, `wire_panel_tests.rs`.
Decisions: A/B follow the wire list order; a stated termination replaces the ferrule only at that end; bare is never on the crimp list; legacy `end_processing` text is parsed into a kind (unknown text = Other). Spares use the ferrule standard percentage. Torque/lug-size checks are not built.
