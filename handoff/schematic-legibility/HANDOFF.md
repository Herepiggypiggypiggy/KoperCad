# Handoff: schematic legibility

- Floor and hiding: `crates/novo-canvas/src/declutter.rs`, used by `Canvas::paint_labels`
  (`min_text_px`, 9.0). `Label::secondary()` marks notes, wire and cable numbers and
  title block captions. Labels at or above the floor are never touched; PDF export does not use it.
- E1: `Viewport::fit` already fits the smaller of width and height (margin 24); the cause of
  small text was the A3 sheet scale, so the fix is the floor, not the fit.
- Colours: `CanvasPalette::sheet_text` -> `ModelStyle::sheet_text_color`.
- Dock: `NovoApp::tool_hint()` (dock.rs) feeds the status bar; the dock shows the hint itself
  only when the status bar is off. Lesson anchors `dock.hint` and `dock.proposals` now sit in the bar.
- Fit (#812): `Viewport` default fit frames `Scene::content_bounds()` with `FIT_CONTENT_MARGIN` 64 and
  zoom cap `FIT_CONTENT_MAX_ZOOM` 6 (`novo-canvas/src/widget.rs`, `Fit::Content`/`Fit::Sheet`).
  `view.fit_sheet` ("Fit whole sheet", View menu, Ctrl+K) shows the whole sheet. Learn replay
  tests that click world points call `zoom_to_sheet()` or centre the point first.
- Notes (#818): `novo-canvas/src/note_place.rs` (candidate list, `Obstacles`) and
  `model.rs` (`refresh_notes`, `NoteSlot`). Notes are placed in symbol order and avoid wires,
  symbols and earlier notes; `apply` re-places only notes near touched boxes. A note the user drags
  is stored as symbol properties `note_dx`/`note_dy` (mm from the default place) and is then an
  obstacle itself. Drag: `LabelSource::Note` in novo-editor; `edit.reset_note` ("Reset note
  position") clears it.
