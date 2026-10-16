# 3D view (WS-29): handoff

Updated 2026-10-15 19:45 UTC.

## State
- Repo herepiggypiggypiggy/kopercad. Branch `claude/project-thread-8laqbm` (the only branch this thread may push to; after a merge, `git fetch origin main && git merge origin/main`, since forced restarts are denied here).
- Merged: #395 design doc, #439 skeleton, #481 renderer (offscreen wgpu, egui widget `widget::View3d`, `orbit` and `window` examples, GPU tests, wires as tubes with picking). Also on main from other threads: scene model `build` (#391), look values, feel, cube, labels, governor (#457).
- Overlays (`set_overlays`) and the door driver (`set_door`) are on main (#647). The follow-up PR of 2026-10-16 adds layers (enclosure, door, wires), routed wires, the enclosure's own shell and door, the 3D toolbar with door angle, commands `view.3d_door/_shell/_wires` and keys Ctrl+6 / Shift+D (UI in `crates/novo-app/src/app/view3d_controls.rs`; selection, inspector and view presets are the UI integration thread's, #655).
- Open work for this thread (follow-up PR): hinge/door animation driver, overlay colours in widget.rs moving into Look/palette, "?" badge vs dashed edge for guessed size (doc), hover tooltip and label chip, feel constants from the look thread's feel module. The look thread said it will wire cube, labels, outlines and governor into widget.rs: agree the split with it before touching widget.rs.
- UI entry point (section 12): third face of the Cabinet canvas (Plate / Table / 3D) and command `cabinet.view3d`; host wiring belongs to the UI integration thread (session_01G52sVY5N34jPsrkqWukApR), who got the exact calls: `widget::install`, `View3d::new`, `View3d::show`. No menu entry until hands-on tested.
- Not done: no measurement on a real GPU (owner's Windows laptop: `cargo run --release -p novo-view3d --example window`); coordinator PR to move dev-dependencies naga and pollster into `[workspace.dependencies]`.

## Resume
```
git fetch origin && git checkout claude/project-thread-8laqbm
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-view3d
```
GPU tests need an adapter; in a cloud container `apt-get update && apt-get install -y mesa-vulkan-drivers vulkan-tools` gives lavapipe.

## People
- Integration lead: cse_01FiLvRBKEcNNHbQEGhYyL1S. Design reviewer: cse_0114emQ5fSFrzcALbR4S18TU. Main reviewer: cse_016C9nTT8cN7voW8t1NQUG4W.
- After every push message the reviewer with the head SHA. After READY, no more pushes to that PR.

## Traps
- The crate list and `[workspace.dependencies]` are the coordinator's; do not edit them.
- No build dispatch until the owner asks.
