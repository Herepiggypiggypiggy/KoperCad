# Movable panels handoff

Use `panel::Panel` for a card on the canvas (builder: side, top, fit, fixed_open, bare, scroll_bar_always) and `panel::dialog` for a window a command opens. Position and size live in egui area memory (persisted by eframe); a drag marks the panel "moved" so it stops following its corner. `view.reset_panels` clears both.

Same panel id means the same remembered place: the two heat legends share "heat_legend".

Not verified: Windows by hand, restart persistence by hand. The Linux tests cover placement, drag then reset and the 3D overlap at 1366x768 and 900x600.
