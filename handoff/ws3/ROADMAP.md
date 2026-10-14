# WS-03 Canvas: roadmap

Scope: crates/novo-canvas. GPU canvas (wgpu 30 + egui 0.36): pan/zoom, grid and snap, layers,
hit testing, selection, rendering model pages (PageScene adapter over novo-core). No model
changes (WS-02 owns novo-core), no editing tools (WS-06/WS-08 own them).

## Done
- PR #4 (merged d91e961): engine, renderer, widget, PageScene, demo + stress examples,
  review fixes, right-drag threshold, Escape ladder, themable ModelStyle/CanvasStyle.
- PR #28 (open): per-label picking (Hit::label), set_related highlight, junction glyphs
  derived from wire topology, pixel-snapped viewport, one drawing per symbol type
  (SymbolArt::cache_key), SymbolArt::labels + tag_anchor (WS-06 request). No key bindings
  for orientation (Tab = next proposal, R/X orient; WS-08 owns placement).

## Ready locally (next PR)
- Marks on the drawing (DESIGN.md §5): src/marks.rs + widget painting.
- Page frame: column numbers and title block (DESIGN.md §3 fields).

## Next, in order
1. Drive #28 to green and through review (reviewer cse_016C9nTT8cN7voW8t1NQUG4W).
2. New PR from main after #28 merges (code ready in next-pr/): page frame per WS-06 request:
   column numbers 1..format.columns top and bottom (match PageFormat::column_at), title block
   bottom-right (ProjectInfo.name, page name + title, "page n of m" from pages_in_order,
   page property "structure"). Mark frame dirty in apply() on page or info changes
   (ChangeSet.info, any Page in added/removed/modified since page count/order changes "n of m").
3. Once WS-10 #31 merges: draw XrefIndex::labels_on_page(page) as extra labels on each
   symbol item (cross-reference colour; Label.warning -> warning colour); selection tint via
   CanvasView::set_related(XrefIndex::highlight(sym).on_page). Diff by symbol id.
4. After #35 (WS-02): draw/refresh Text records; drop canvas-drawn tags once DeviceTag labels
   exist; refresh potential labels when a wire's potential changes.
5. DONE (release thread): Quadro P1000, 2400x1350, 10k objects: 1135 fps avg, 222 fps 1% low.
   Gate 0 passed. Was: integration lead runs `demo --stress 10000 --seconds 10` from the
   experimental build (writes stress-result.txt). Gate 0 done criterion.

6. 100k objects stutter: 113 fps avg, 38.5 fps 1% low on the P1000. Suspect: label re-layout
   burst when the quantised font size steps while zooming (paint_labels). Measure; spread
   galley re-layouts over frames (budget) or coarser size steps.

## Open questions
- Pin designation text on pins (connection labels): canvas option or SymbolArt::labels? (WS-06)
- INSTALLED renderer flag is process-wide; per render state if multiple native windows.
