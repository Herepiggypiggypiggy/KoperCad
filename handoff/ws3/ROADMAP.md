# WS-03 Canvas: roadmap

Scope: crates/novo-canvas. GPU canvas (wgpu 30 + egui 0.36): pan/zoom, grid and snap, layers,
hit testing, selection, rendering model pages (PageScene adapter over novo-core). No model
changes (WS-02 owns novo-core), no editing tools (WS-06/WS-08 own them).

## Done (merged)
- #4 engine, renderer, widget, PageScene. #28 label picking, related highlight, junction glyphs,
  pixel snap, shared symbol art. #47 marks and page frame/title block. #93 xref notes in the
  scene. #103 wgpu optional (feature `gpu`). #109 generated sheets (SheetContent). #120 perf
  (100k: show() p99 20.9 -> 1.2 ms). Gate 0 passed.

## Next, in order
1. Drawing PR: junction dot only at 3+ wires, NoteAnchor, keep_color, strip names (F5).
2. Reveal PR: CanvasView::reveal for jumps (next-pr/canvas-reveal).
3. Phase 3: what the Cabinet canvas (WS-15) needs from the engine (fills, dimensions, DIN rail
   snapping); ask WS-15 before building.

## Open questions
- Pin designation text on pins (connection labels): canvas option or SymbolArt::labels? (WS-06)
- INSTALLED renderer flag is process-wide; per render state if multiple native windows.
