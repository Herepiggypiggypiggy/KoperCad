# Busbar impedance and current sharing: handoff
Branch `claude/project-thread-1edw2o`. Engine `crates/novo-emc/src/busbar.rs`, UI `crates/novo-app/src/app/calc_busbar.rs` (Calculations tab), doc `docs/busbar-impedance.md`.
Done: filament engine, 14 tests, Busbars tab, model label row, anchor `calc.tab.busbars`.
Next: pick bars from the Cabinet (busbar devices), Cabinet check on unequal sharing, feed loss per filament to the busbar heat solver, force estimate input.
Resume: `cargo test -p novo-emc busbar`; `cargo clippy -p novo-app --all-targets -- -D warnings`.
