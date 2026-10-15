# Handoff: Device models and animation

#346 merged. Follow-up PR from main (same branch name): contact blades, timer ring, `Settings.reduce_motion`. First PR: design doc, `novo-sim/src/devices.rs` with `tests/devices.rs`, `novo-app/src/app/live_anim.rs`, one hook in `live.rs` (`LiveView.anim`, `anim.paint`).

How it works: `device_states(project, netlist, &Settled)` reads the settled solution; lamps and motors are recognised by library key (`iec/signalling/lamp*`, `iec/motors/motor_*`). The app eases the values per frame and paints over the canvas. Nothing is stored in the project.

Decisions
- Motor direction: trailing digit of the potential name (L1 L2 L3 and rotations = forward), else the order of potential ids. PE pin ignored. 6-terminal motor uses its first three pins; Dahlander treats each group of three as a speed.
- DC motor runs reverse when A1 is DC negative.
- Coupling, latching and wire tint stay with Potentials; do not edit `coupling.rs` from here.

Resume: `cargo test -p novo-sim -p novo-app live_anim`, gate: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
Talk to: Potentials cse_01LfvbEvxsdpj175azTV38EX, integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S, reviewer cse_016C9nTT8cN7voW8t1NQUG4W.
Not hands-on tested in the real window yet (cloud session, no display).
