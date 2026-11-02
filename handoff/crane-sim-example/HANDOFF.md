# Crane simulation example: handoff

Branch `claude/project-thread-9klcqs` (kopercad). Code: `crates/novo-app/src/app/example_crane_sim.rs`
(receiver pages, machine properties, controller), `example_crane.rs` (`draw(tx, sim)` and the
`bus_feed` helpers), `example_kit.rs` (`Kit::ip`). Design: `docs/design/crane-sim-example.md`.

Resume: `cargo test -p novo-app --lib example_crane_sim` and `example_gallery_tests`.

Traps found
- A potential feed is a SOURCE, whatever its kind. Gating a rail needs an interruption point name.
- Pins of the receiver block must be renamed per symbol (`E1/E2`, `M1/M2`); a second `Main` class
  symbol on the same device is refused, so receiver symbols are placed with explicit roles.
- `outputs_6` pins are optional so spare outputs do not warn in Check.
- Device-state tests need `live.solved(project, 0.0)` once before pressing controls.
