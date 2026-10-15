# Parts library at scale: handoff

Branch `claude/project-thread-kmmhnd` in kopercad. First PR: schema v2 + indexed catalog (see PROGRESS.md for the number).
State: schema and scale done and tested (`crates/novo-parts/tests/schema_v2.rs`). Nothing half-done.
Resume: `cargo test -p novo-parts`; perf test asserts 100 ms in release only (`--release`).
Coordinator session for lane threads: session_01PgJyn3Yh6bBF3XNhYuLmpc.
Traps: `CatalogPart` has many fields now; construct with `..Default::default()`. Catalog fields are private; use `upsert`. Unknown stays unknown: never default a price, lifecycle or terminal position.
