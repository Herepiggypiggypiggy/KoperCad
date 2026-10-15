# 3D view (WS-29): handoff

Updated 2026-10-15 18:50 UTC.

## State
- Repo herepiggypiggypiggy/kopercad. Branch `claude/project-thread-8laqbm` (the only branch this thread may push to; restart it from main after each merge).
- #395 (design doc) merged. PR 2 (draft): crate skeleton, headless only: scene contract, camera, picking, culling, demo cabinet.
- Next PR: renderer, egui widget, examples (`orbit`, `window`), GPU tests, measured numbers in the doc. Developed on local branch view3d-spike (not pushed).
- Decisions already made (details in the doc): own small wgpu renderer drawing offscreen with its own depth, flat scene with instanced boxes, CPU picking and culling, render on demand, interactive and settled quality tiers, labels and view cube drawn by egui over the image.

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
