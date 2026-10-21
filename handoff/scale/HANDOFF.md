# Scale handoff (2026-10-21)

#1173 merged. Follow-up PR from main on `claude/project-thread-dwthx1`: varied generator, labelled redraw timing, heat before/after table. Files: `crates/novo-tests/src/big.rs` (generator), `examples/scale_bench.rs`, `tests/scale_budget.rs`, `scripts/scale-bench.sh`, fix in `crates/novo-cables/src/screen.rs`, heat changes in `crates/novo-thermal/src/{thermal,flow}.rs`.
Resume: `scripts/scale-bench.sh --size large --only generate,save,open,search,page,edit,number,design,report,autopilot,pdf,route,3d` (heat steps take minutes). Results and ranked leftovers: RESULTS.md.
Traps: the heat step needs a cabinet with an enclosure (generator sets one). `--only` matches name substrings. Netlist is not cached, so any new loop that calls `project.netlist()` per item is quadratic. Thermal change touches numerics owned by the thermal/CFD threads: ask a numerics reviewer (R1) to confirm the settle rule.

Varied generator: `BigSpec::new(n)` (varied, default) or `.uniform()` / `--uniform`. Examples: `validation_temps` (novo-thermal), `heat_dense_temps` (novo-tests, `--uniform`). Before/after needs a git worktree of `158dfc1^` with the same example copied in.
