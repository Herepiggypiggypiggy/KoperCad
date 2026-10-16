# Parts library at scale: handoff

Branch `claude/project-thread-kmmhnd` in kopercad. #512 (schema v2, indexed catalog) merged. Second PR: Parts window detail shows the v2 fields, `Catalog::retain`, review nits.
Resume: `cargo test -p novo-parts -p novo-app -p novo-interop`; perf test asserts 100 ms in release only.
Next: see "Import pipeline plan" in ROADMAP.md. Paused: Tadjiro asked to stop starting new work (2026-10-15 ~19:50Z).
Coordinator session for lane threads: session_01PgJyn3Yh6bBF3XNhYuLmpc. Protection thread (cse_01CQXymGH5kY1GfABR1JHePK) reads `novo_parts::eng` keys.
Traps: construct `CatalogPart` with `..Default::default()`. Keep large CatalogPart values boxed inside enums (clippy large_enum_variant). Unknown stays unknown: never default a price, lifecycle, source or terminal position.
