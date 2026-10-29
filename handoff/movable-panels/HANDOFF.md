# Movable panels handoff

Use `panel::Panel` for a card on the canvas (builder: side, top, fit, fixed_open, bare, scroll_bar_always) and `panel::dialog` for a window a command opens. Position and size live in egui area memory (persisted by eframe); a drag marks the panel "moved" so it stops following its corner. `view.reset_panels` clears both.

Same panel id means the same remembered place: the two heat legends share "heat_legend".

Not verified: Windows by hand, restart persistence by hand. The Linux tests cover placement, drag then reset and the 3D overlap at 1366x768 and 900x600.

2026-10-28 additions: `panel::modal(ctx, "name", "Title", [w, 0.0], |ui| ..)` replaces `egui::Modal` (same `.inner` and `.should_close()`; it is centered until moved, backdrop click and Escape close it when `should_close` is asked). `panel::Float::new(name, canvas, pivot, at).show(ctx, ..)` is for a card without a title bar (dragged by its background). New windows must not call `egui::Window::new`, `egui::Modal::new` or a raw `egui::Area::new`: the test `no_fixed_window_is_built_outside_the_panel_helpers` fails. A dialog's content should not call `ui.set_width` (that stops the window being resized); give the width as the default size.
