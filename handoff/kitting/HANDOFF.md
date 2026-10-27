# Kitting and stock optimiser: handoff

State 2026-10-27: PR from branch `claude/project-thread-oblbwd` (see PROGRESS.md for head SHA and review state).

Code:
- `crates/novo-reports/src/kitting/cutting.rs`: `plan(material, demands, held)`. Branch and bound over best-fit-decreasing; cost = purchased length, then bars; `budget_hit` when the node limit (300k) stops it; `proven_optimal()` when it buys the lower bound. Kerf trick: a piece needs length + kerf, a bar holds length + kerf.
- `settings.rs`: `KitSettings` (JSON in project info property), `OnHand::parse` (lines `rail 1200 x2`, `duct 50x60 1500`, `wire 1.5 BK 37.5 m`, `have A1.5-10 x40`).
- `plan.rs`: `kit_plan(project, settings)`. Rails and ducts of every plated cabinet in cabinet-tag order share bars; wires are hookup jobs from `bench_jobs` (cable cores are cut from their cable, listed by metres); allowance is held back by packing wires as if longer; ferrules from `ferrule_counts` with the ferrule standard's spares; lugs are ends with stated processing; screws are one per drill-table hole; labels from `layout_labels`.
- `report.rs`: `Kitting::build(plan, revision)`, report id `kitting` (not a default page). Sections `pick-prepare|mount|wire|label`, `saw-<material>`, `reels`, `offcuts`, `shopping`, `notes`.
- `crates/novo-app/src/app/kitting.rs`: Kitting tab (draft settings, plan cache), `export_kitting`, `kitting_pdf(quiet)`; wired in `bench.rs` (4th tab, `Act::ExportKitting`, `Act::SaveKitting`), `workbench.rs` (`export.kitting`), `workshop.rs` (`3 Kitting list.pdf`), workflow `commands.rs`, `menus.rs`, `language.rs`.
- Tests: `cutting/tests.rs` (15, includes an exhaustive-search comparison), `settings.rs`, `crates/novo-reports/tests/kitting.rs` (13), `crates/novo-app/src/app/kitting_tests.rs` (6).

Decisions:
- Cost is length, not price: no supplier prices in the model.
- Rails are cut at the length the layout gives; the older Cabinet > Cut list can still trim rails to save a bar.
- Reel length, kerf, allowance, packs are planning values; the report notes say so. Counts come from the drawing.
- Wire lengths that are estimates make the reel rows say "estimate" and a problem line is added.
- The merge of stock lines into BOM lines needs an order number given in the settings (`Item = order number`).

Traps: cloud disk fills during workspace builds (`scripts/free-disk.sh`, `CARGO_PROFILE_DEV_DEBUG=0`). New anchors need `learn.rs` and `learn_labels.rs` entries and `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`.

Resume: `cargo test -p novo-reports kitting`, `cargo test -p novo-app --lib kitting`, `scripts/qa.sh run qa/scenarios/32*.qa`.
Reports: `/mnt/project-files/qa/kitting3`, `kitting4`.
