# Scale handoff (2026-10-21)

Branch `claude/project-thread-dwthx1`, one PR. Files: `crates/novo-tests/src/big.rs` (generator), `examples/scale_bench.rs`, `tests/scale_budget.rs`, `scripts/scale-bench.sh`, fix in `crates/novo-cables/src/screen.rs`, heat changes in `crates/novo-thermal/src/{thermal,flow}.rs`.
Resume: `scripts/scale-bench.sh --size large --only generate,save,open,search,page,edit,number,design,report,autopilot,pdf,route,3d` (heat steps take minutes). Results and ranked leftovers: RESULTS.md.
Traps: the heat step needs a cabinet with an enclosure (generator sets one). `--only` matches name substrings. Netlist is not cached, so any new loop that calls `project.netlist()` per item is quadratic. Thermal change touches numerics owned by the thermal/CFD threads: ask a numerics reviewer (R1) to confirm the settle rule.
