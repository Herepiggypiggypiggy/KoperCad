# Responsiveness results

Project: the varied 600-page generator (2,676 devices, 8,513 wires). Release build. One cloud container, no GPU (Xvfb + software Vulkan). One run each.

| Measure | Before | After |
| --- | ---: | ---: |
| UI pass per frame, 1366x768, standing still (mean) | 110 ms | 8 ms |
| UI pass, panning | 102 ms | 9 ms |
| UI pass, zooming | 106 ms | 9 ms |
| Whole frame (software rasteriser included), 1366x768 | 135 ms | 30 ms |
| UI pass, 1920x1080 | 134 ms | 30 ms |
| Whole frame, 1920x1080 | 171 ms | 70 ms |
| `Project::netlist()` repeat call | 17-27 ms | 0 ms (shared) |
| Machine options: evaluate per edit | 33 ms | 17 ms |
| Machine options: project copy for the documents | 73 ms | 53 ms |
| Design check after an edit (big projects) | 1.0 s on the UI thread | on a worker thread, last answer stays on screen |

Cause of the frame cost: `suggest_terminals` ran on the whole project about ten times per frame (stack samples: 14 of 14). Now once per revision (`Project::derived`).

The whole-frame figure is the CPU rasteriser, not a graphics card; the UI pass is the program's own cost and does not depend on the GPU. A real-GPU figure needs the scenario run on a PC with a GPU.

## How to find the next hot spot
Release test build, then sample stacks while it runs frames:
`KOPERCAD_SCALE_FILE=target/scale600.novo cargo test --release -p novo-app --lib frame_cost -- --ignored --nocapture` (set `KOPERCAD_FRAMES=300` to run long) and `gdb -p PID -batch -ex "thread apply all bt 40"` a dozen times.
Heat and CFD already run on a worker thread with cancel (`heat_run.rs`); not changed.
