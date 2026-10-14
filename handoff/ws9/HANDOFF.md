# WS-09 handoff

Branch: claude/ws09-pdf-export, head 95d4749, on main f6ee2f1. PR #49, sent to the reviewer
(session_01XEdxpvQFh4HWmurh72tKwp). Written by a coordinator helper agent; no thread of its own yet.

## State
- crates/novo-pdf: src/lib.rs (API, page loop, outline, links), src/draw.rs (content stream:
  mm transform, primitives, Béziers, labels), src/font.rs (WinAnsi + Helvetica widths),
  tests/gate1_pdf.rs (11 lopdf tests), examples/gate1.rs.
- Workspace deps: novo-pdf, pdf-writer = "0.15". Dev-dep lopdf 0.45 (default-features off).
- Nothing half-done.

## Seam
PageScene::new(project, page, art) + set_style(print_style()); scene().iter() sorted by
(layer order, ItemId); item primitives/labels; scene().item_bounds() for link rects;
model_id() for link sources. novo-canvas is a path dep with default-features = false,
features = ["model"] (workspace deps cannot turn default features off); wgpu still compiles
but no GPU is touched.

## Resume
```
git worktree add $S/wt-ws09 origin/claude/ws09-pdf-export
export CARGO_TARGET_DIR=$S/target-ws09 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p novo-pdf; cargo run -p novo-pdf --example gate1 -- $S/gate1.pdf
pdftoppm -png -r 80 -f 1 -l 1 $S/gate1.pdf $S/page   # visual check
```

## Decisions
- Helvetica, not embedded Inter, for v1; non-WinAnsi characters print as '?'.
- Page set keeps project order and dedups; an unknown PageId is an error, drawing problems never are.
- Empty export gives one blank A3 landscape page so readers open it.
- No dates or file id: the same project gives the same bytes (tests rely on it).
- Content streams uncompressed (greppable in tests).

## Traps
- Don't push after READY; follow-ups go in a new PR from main.
- novo-canvas is WS-03's crate: keep any change there additive and minimal.
