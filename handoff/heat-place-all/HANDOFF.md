# Heat-aware Place all: handoff

Branch `claude/project-thread-g1cq89` (repo kopercad), based on main bb4fc6f. Not stacked on #1365.

Code: `crates/novo-layout/src/heat.rs` (search), `rules.rs` (Tweaks threaded through plan_around/plan_once, `heat` rule line, `Spec.limit_c`, `Decision.heat`, `Explanation.heat`), `check.rs` (zone override), app: `cabinet.rs` (message), `cabinet_rules.rs` (check box, `set_heat_line`).
Tests: `crates/novo-layout/src/heat.rs` unit tests, `crates/novo-tests/tests/heat_place_all.rs` (2D model), `cabinet_rules_tests.rs` (app), QA scenario `qa/scenarios/78-heat-aware-place-all.qa` with fixture `qa/fixtures/heat-place-all.novo` (made by `heat_place_all --save`).
Validation tool: `cargo run --release -p novo-tests --example heat_place_all -- <file.novo> [--detail]` or `-- --synthetic`.

Results (2D model, planning values, not validated): synthetic cabinet 8 over -> 0 over, tightest -9.1 -> +0.8 K. CV-200 (/mnt/project-files/audit/real-machine/heat-layout/cv200-with-assumptions.novo): 2 over -> 2 over, tightest -13.0 -> -13.2 K (within the model's ~1 K wobble), mean margin +9.2 -> +10.9 K, air at top 47.7 -> 46.6 C, the heat-aware run is "not converged" in the model.

Traps: any new `Spec { .. }` literal needs `limit_c`. `Tweaks.rank` only reorders within (zone, kind). The search is deterministic (no hash order leaks); keep it so. Moves that add rows beyond +3, lose the trunk or the separation are rejected.
