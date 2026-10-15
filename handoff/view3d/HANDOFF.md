# 3D view (WS-29): handoff

Updated 2026-10-15 19:15 UTC.

## State
- Repo herepiggypiggypiggy/kopercad. Branch `claude/project-thread-8laqbm` (the only branch this thread may push to; restart it from main after each merge).
- Merged: #395 (design doc), #439 (crate skeleton: scene contract, camera, picking, culling, demo cabinet).
- Renderer PR (draft, this branch): offscreen wgpu renderer (`render`, feature `gpu`), egui widget (`widget`, feature `egui`, default), `orbit` and `window` examples, GPU tests, measured numbers in the doc, wires as tubes with picking and marquee. Review nits from #439 are folded in (wires pickable, marquee across the eye plane, NaN note).
- Stacked on the skeleton by other threads: scene model `build` (#391), look values and feel (#457), part geometry (#449, novo-interop).
- Decisions already made (details in the doc): own small wgpu renderer drawing offscreen with its own depth, flat scene with instanced boxes, CPU picking and culling, render on demand, interactive and settled quality tiers, labels and view cube drawn by egui over the image.
- UI entry point (section 12): third face of the Cabinet canvas (Plate / Table / 3D) and command `cabinet.view3d`; the host wiring belongs to the UI integration thread, no menu entry until hands-on tested.
- Not done: no measurement on a real GPU (needs the owner's Windows laptop: `cargo run --release -p novo-view3d --example window`), hinge animation of the door, view cube and label layout (look thread, #457).

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
