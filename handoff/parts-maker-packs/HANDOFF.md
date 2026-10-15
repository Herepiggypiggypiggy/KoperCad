# Handoff: Parts: maker packs

Branch: `claude/project-thread-w953l7`, PR #547 (base main; #512 is merged).

State: recipes, mapping engine, import wiring, Parts window steps and golden tests are in. Run `cargo test -p novo-parts` and `cargo test -p novo-app parts_import`.

Where things are
- `crates/novo-parts/src/recipes/`: `makers.rs` (the nine recipes + `GENERIC`), `apply.rs` (rule table, `MakerRecipe::apply`), `quantity.rs` (units and ranges).
- `import.rs`: `ImportJob` runs the recipe over BMEcat/CSV/ZIP parts (`ImportOutcome.recipe`); `DownloadsWatcher::for_maker`.
- `crates/novo-app/src/app/parts_import.rs` and `parts.rs`: steps and warning in the Waiting strip, summary lines after import.
- `crates/novo-parts/tests/golden/`: synthetic samples + `sample.expected.json`; `UPDATE_GOLDEN=1` regenerates.

Traps and decisions
- Samples are synthetic (no network to makers here). Only WAGO and Phoenix routes are owner-captured; the rest say unverified in the UI and use the maker's home page.
- Nothing is guessed: no price without currency, no power loss given per pole, terminal names 1..N only for terminal blocks with a stated connection count (warned).
- Raw features stay in `data`; mapped values are added with `DataSource::MakerDatasheet`.
- `catalog.rs` untouched (schema lane). `FileKind::Csv` added in `library.rs`.
