# WS-03 Canvas: handoff

## State (2026-10-15 03:20 UTC)
- Repo github.com/Herepiggypiggypiggy/NovoElectric, crate crates/novo-canvas.
- One pinned branch: claude/canvas-engine-ymvwu2. So PRs go up one at a time.
- Merged: #4, #28, #47, #93, #103, #109 (sheets), #120 (perf), #126 (drawing, strip runs as proximity groups), #144 (jump reveal, SheetText.bold), #154 (bold face).
- Open: nothing.
  Reviewer B (session_01QSnXFBDRyvwwzhT1S1gFGN) has correctness; design reviewer (session_0114emQ5fSFrzcALbR4S18TU) anything visual.
- Next, ready locally (ffca612 on top of #120; patch in next-pr/canvas-drawing/): canvas-drawing
  - JunctionGlyph::Plain (<3 wires, no dot); NoteAnchor mapped from xref LabelAnchor (#102);
    Label::keep_color for warnings; strip-name labels; un-ignores F5 gap_pdf_names_the_terminal_strip.
  - Send to reviewer B + design reviewer; tell the tester (session_01LMMcDMt9qaHXoSXy7iDu58) and WS-10.
- Republish: `git fetch origin main && git checkout -B claude/canvas-engine-ymvwu2 origin/main && git am next-pr/<b>/*.patch`.
  Then checks, push (force-with-lease if the branch's PR merged), PR, subscribe, and message the reviewer + lead.
- Still to do:
  - fit_text ADVANCE 0.6 vs PDF Helvetica widths (novo-pdf could re-cut); shared Label->SymbolNote helper.
  - N6 artwork text belongs to WS-06 (LibraryArt::labels).

## Xvfb click finding
Does not reproduce in code. In this container Xvfb ignores XTEST and XWarpPointer: the pointer stays at the screen centre, and xdotool clicks never arrive.
Real-Windows check is still the owner's.

## Resume
```
export CARGO_TARGET_DIR=/home/claude/target-main CARGO_INCREMENTAL=0   # disk is tight: one target dir
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run --release -p novo-canvas --example demo -- --stress 100000 --seconds 10
```
CI (Actions) is down: paste local results in the PR.

## Decisions
- World mm, y down; canvas angles clockwise; item ids = novo-core raw ids.
- Canvas never depends on novo-xref / novo-reports: callers convert into SymbolNote / SheetContent.
- Notes: above the top right of the symbol ink, tag height, ModelStyle.note_color / note_warning_color.
- Ink beats text in picking; terminals show only their name; junction dot counts wires.
- Canvas binds no letter keys and no Tab; Escape ladder via escape_with.

## Traps
- wgpu 30.x to match egui-wgpu 0.36. Clippy 1.99 in CI.
- egui test contexts: clear textures_delta. Don't call Response methods inside ctx.input closures (deadlock panic).
- `pkill -f <pattern>` kills your own shell when the pattern is in the command line; use pidof.
- Disk: NovoElectric/target was 18 GB; use the single target dir above.

## People
- Integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S (merges).
- Design reviewer session_0114emQ5fSFrzcALbR4S18TU. Coordinator session_018btnHEHYdedX4jueCivWjT.
