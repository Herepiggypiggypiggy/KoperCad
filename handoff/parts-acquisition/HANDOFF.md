# Handoff: parts acquisition

Branch `claude/parts-cad-match` (from main cf49f8d). P1 (CAD file matching) is in review; P2 and P3 not started, see PROGRESS.md and ROADMAP.md.
- Engine: `crates/novo-parts/src/cadmatch.rs` (identity, family ranges, doc ids, `CadLinks`, `rank`, `assess_part`, `decide_drop`, `match_batch`). Real-file test: `tests/cadmatch_real.rs` with `KOPERCAD_REAL_CAD_DIR` (the Schneider LC1D09 files are on Tadjiro's PC, `Documents\KoperCAD-tests\work\claude-parts\cad\schneider`; never commit them).
- App: `crates/novo-app/src/app/parts_cad.rs` (panel), `parts.rs` (drop logic, `drop_on_target`, `judge_downloads`), `parts_import.rs` (`import_checked`, `Checked`).
- Recipes: `MakerRecipe::account_note`.
Traps: `identity_of` uses `measure_step` (PR 1563 merged). PR 1562 changes `user_catalog_path`; the library root follows it.
