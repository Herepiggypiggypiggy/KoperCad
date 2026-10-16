# Handoff: schematic legibility

- Floor and hiding: `crates/novo-canvas/src/declutter.rs`, used by `Canvas::paint_labels`
  (`min_text_px`, 9.0). `Label::secondary()` marks notes, wire and cable numbers and
  title block captions. Labels at or above the floor are never touched; PDF export does not use it.
- E1: `Viewport::fit` already fits the smaller of width and height (margin 24); the cause of
  small text was the A3 sheet scale, so the fix is the floor, not the fit.
- Colours: `CanvasPalette::sheet_text` -> `ModelStyle::sheet_text_color`.
- Dock: `NovoApp::tool_hint()` (dock.rs) feeds the status bar; the dock shows the hint itself
  only when the status bar is off. Lesson anchors `dock.hint` and `dock.proposals` now sit in the bar.
