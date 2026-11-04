# Queries thread handoff

Branch `claude/project-thread-lqvdxl` of kopercad. Code: `crates/novo-reports/src/query/` (value, parse, tables, eval, builder, preset), editor `crates/novo-app/src/app/reports_queries.rs`, CLI verb in `crates/novo-cli/src/lib.rs`. Docs `docs/QUERIES.md`.
Tests: `cargo test -p novo-reports --test query`, `cargo test -p novo-cli --test cli query_reads`, unit tests in query/*.rs.
Presets live in add-on data `reports/presets`; keep that name (stored in users' files).
Trap: `standard_reports()` now has a 15th report (`queries`); tests that count reports may need the new id.
