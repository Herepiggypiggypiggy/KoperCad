# 3D view (WS-29): handoff

Updated 2026-10-16 UTC.

## State
- Repo herepiggypiggypiggy/kopercad. Work on a new branch from main per PR (a merged PR cannot be reused; the old branch name is stale).
- On main: design doc, crate, renderer, egui widget (`widget::View3d`), overlays, door driver, layers, routed wires, shell and door from the enclosure, toolbar (layers, door angle, shadows, dimensions, move, stats), wire colours, frame statistics (`widget::render_stats`, View > 3D), dimensions (`dimension.rs`), frame governor connected to the widget. Selection, inspector, move and Heat in the 3D face are other threads' (UI integration, CFD-F, 3D look).
- Commands: view.3d, view.3d_door, view.3d_shell, view.3d_wires, view.3d_shadows, view.3d_dimensions, view.3d_stats; keys Ctrl+6 and Shift+D.
- Not done: no measurement on a real GPU yet (steps in the design doc, "Measuring on your own machine"; the owner copies the panel's lines). Colour by potential is in for PE, neutral, 0 V and positive DC; a line conductor stays grey until the model says power or control.

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
