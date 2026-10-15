# Thermal engine handoff

State (2026-10-15 20:10 UTC): engine complete and tested on branch
`claude/project-thread-cegdth` (kopercad), draft PR #505. Not yet READY.

Resume
- `cargo test -p novo-thermal` (about 5 s), `cargo clippy -p novo-thermal --all-targets -- -D warnings`.
- `cargo run -p novo-thermal --release --example demo` (add `--json` for fields).

Layout of the crate
- `model.rs` inputs, `result.rs` outputs (contract with the visuals thread, keep additive).
- `setup.rs` rasterises a scenario (blockage, plate, wall conductances, devices, ports).
- `flow.rs` pressure based airflow; `thermal.rs` coupled temperature system and the
  flow/temperature fixed point (relaxation cut when the flow swings); `report.rs` result,
  margins, hot spots; `advice.rs` tried recommendations; `linalg.rs` CG and BiCGSTAB.

Decisions
- 2D depth-averaged slice, honest about being a model; band from favourable/unfavourable corners.
- Advice runs on a 30 mm grid in parallel threads.
- Root Cargo.toml got `[profile.dev.package.novo-thermal] opt-level = 3` (tests are 30x faster).
- Crate registration (CLAUDE.md table and `[workspace.dependencies]`) is the integration lead's.

People
- Visuals thread cse_01T9zCX9G6QRbuYHDu4SzExL reads `ThermalResult`.
- Heat model thread cse_017SJTgarPtDUjdGhEdX3fug owns #406 (loss-density map).
- Integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S registers the crate and merges.

Traps
- y points down; rising air has vy < 0.
- Do not add an anchor to the pressure system: it leaks mass and breaks the energy balance.
- Warm starts reset the fan/vent port arrays (`Flow::reset_ports`).
