# Handoff: parts acquisition

Branch `claude/parts-cad-match` (from main cf49f8d). P1 (CAD file matching) is in review; P2 and P3 not started, see PROGRESS.md and ROADMAP.md.
- Engine: `crates/novo-parts/src/cadmatch.rs` (identity, family ranges, doc ids, `CadLinks`, `rank`, `assess_part`, `decide_drop`, `match_batch`). Real-file test: `tests/cadmatch_real.rs` with `KOPERCAD_REAL_CAD_DIR` (the Schneider LC1D09 files are on Tadjiro's PC, `Documents\KoperCAD-tests\work\claude-parts\cad\schneider`; never commit them).
- App: `crates/novo-app/src/app/parts_cad.rs` (panel), `parts.rs` (drop logic, `drop_on_target`, `judge_downloads`), `parts_import.rs` (`import_checked`, `Checked`).
- Recipes: `MakerRecipe::account_note`.
Traps: `identity_of` uses `measure_step` (PR 1563 merged). PR 1562 changes `user_catalog_path`; the library root follows it.

P3 (library bundles): `crates/novo-parts/src/bundle.rs` (`export_bundle`, `Bundle::open`/`preview`/`apply`), app `crates/novo-app/src/app/parts_bundle.rs` (`BundleUi`), design `docs/PARTS-LIBRARY-DESIGN.md` 3f, QA `qa/scenarios/702-parts-library-bundle.qa` with fixture `qa/fixtures/parts/library/example-share.kcpl` (made-up parts). Traps: Share never includes maker parts; Backup needs the "own backup" confirmation; files go in only through `LocalLibrary::store`. P2 (#1578) touches the same More menu and `novo-cli/src/parts.rs`: merge main before merging.
Open, not done: ETIM install on the UI thread; BMEcat `MIME_SOURCE` CAD links into `CadLinks`; price column in the BOM; ETIM xChange JSON.
