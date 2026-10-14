# WS-03 Canvas: handoff

## State (2026-10-14 17:35 UTC)
- Repo github.com/Herepiggypiggypiggy/NovoElectric, crate crates/novo-canvas.
- Branch: claude/canvas-engine-ymvwu2 (harness-pinned branch name; used instead of ws-03-canvas).
- Open PR: #28, head 924be9b, ready for review (not draft). CI was running at handoff.
  Reviewer has not given a verdict yet. Rule: once reviewer says READY, do not push to #28;
  new work goes in a new PR from main (reuse the same branch name only after #28 merges:
  `git fetch origin main && git checkout -B claude/canvas-engine-ymvwu2 origin/main`).
- PR #4 merged as d91e961.
- Ready but unpushed (for the next PR, after #28 merges): local branch canvas-marks =
  #28 head + 2 commits: marks on the drawing (src/marks.rs, DESIGN.md §5) and page frame
  column numbers + title block (DESIGN.md §3 fields). Copies: handoff/ws-03-canvas/next-pr/*.patch.
  Resume: `git fetch origin main && git checkout -B claude/canvas-engine-ymvwu2 origin/main &&
  git am /mnt/project-files/handoff/ws-03-canvas/next-pr/*.patch`, run checks, push, open PR,
  message reviewer with the SHA.
- Waiting on #35 (WS-02 Text records, DeviceTag labels, potentials on wires): after it merges,
  PageScene must draw/refresh Text records, not draw tags twice once DeviceTag labels exist,
  and refresh potential labels when a wire's potential changes (integration lead).
- PR #4 merged as d91e961.
- Ready but unpushed: page frame (column numbers + title block, WS-06 request) as local
  branch canvas-frame (commit 4faa2b8 on top of #28). A copy is saved as
  handoff/ws-03-canvas/frame.patch. After #28 merges: restart the pinned branch from main and
  `git am` the patch (or cherry-pick), run checks, push, open a new PR, notify the reviewer.

## Resume
```
cd NovoElectric && git checkout claude/canvas-engine-ymvwu2 && git pull
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p novo-canvas --example demo                       # model page demo
cargo run --release -p novo-canvas --example demo -- --stress 10000 --seconds 10
cargo run --release -p novo-canvas --example stress            # interactive stress page
```
Headless Linux: apt install mesa-vulkan-drivers libxkbcommon-x11-0; Xvfb :99; DISPLAY=:99.
GPU tests (tests/gpu.rs) skip when no adapter.

## Key files
- src/scene.rs (Scene, spatial hash, pick/box select), src/tessellate.rs, src/render.rs +
  canvas.wgsl (renderer), src/widget.rs (CanvasView egui widget), src/model.rs (PageScene,
  SymbolArt, PlaceholderArt, JunctionGlyph, demo_document), examples/demo.rs, examples/stress.rs.

## Decisions already made
- World units mm, y down; canvas angles clockwise on screen; Rotation::R90 -> -pi/2.
- Item ids are novo-core raw ids (unique across kinds).
- Palette lives in novo_theme (layout thread, PR #20); canvas defaults stay black on white.
  App maps into CanvasStyle / ModelStyle; PageScene::set_style redraws.
- Wiring is pin to pin; junction shapes derived, never placed. Canvas binds no letter keys
  and no Tab. Escape ladder via CanvasView::escape_with; right drag pans after 4 pt,
  right click -> CanvasOutput::context_click.
- Netlist is never built by the canvas.
- Picking: ink beats text (label scored as distance + tolerance). Terminal symbols show only
  the terminal name (test report F2).
- Until docs/DESIGN.md merges: engines and commands only, thin UI.

## Traps
- wgpu must stay 30.x to match egui-wgpu 0.36. naga dev-dep pinned "30".
- CI runs Rust 1.99 clippy; `chunks_exact` on byte buffers triggers a lint, use as_chunks.
- CI does not pass --all-features; `model` is a default feature so it is tested.
- egui test contexts: clear `output.textures_delta` or the drop panics.
- After each push, message the reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) with the head SHA.

## People (session ids, see coordination/threads.md)
- Integration lead / coordinator: session_011PfTJ1QaP1JQJPwXcFnxBE (merges; arbitration).
- Reviewer cse_016C9nTT8cN7voW8t1NQUG4W. WS-06 editor cse_019siuxbyyRNZRgBBww1KocJ.
- WS-10 xref cse_01KrbvfWtr3dmZqL1bwwyKWF. WS-02 model cse_01B1jpjzeDiXvWoBCsv1BWt9.
- Layout/theme cse_01UXbCeQW22y6tv74F2Xvbvm. WS-08 keyboard cse_01TAFG6LPGy2WMwhJYXmxqxF.

## Lead fixes waiting (read first)
See lead-fixes/README.md: junction dot and #35 text drawing fixed locally at a3302fc by the integration lead; ALREADY PUSHED at 18:06 UTC; just `git pull` the PR branch.
