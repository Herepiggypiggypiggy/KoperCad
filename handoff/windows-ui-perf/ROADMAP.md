# Windows UI and performance findings: roadmap

Scope: the UWL and HPR findings of the Windows hardware test 2026-10-28 (reports in `qa/hardware-2026-10-28/` on the project mount).

Done (one PR from main, branch claude/project-thread-5dsxif):
- UWL-01, UWL-05 (Preferences half): a modal dialog kept the size it had before the screen shrank. `appearance.rs` `settle_dialogs_after_resize` resets the egui areas on the frame the screen size changes while a modal is open. Covers all 32 `egui::Modal` dialogs.
- UWL-02: `novo_theme::widgets::segment` draws a focus ring (the frameless segment had none).
- UWL-03: keymap.txt with a byte order mark or in UTF-16 is read.
- UWL-04: submenus (View > 3D has 43 rows) scroll inside the window.
- HPR-03: start screen scrolls.
- HPR-05: Commissioning checks list lays out only the rows in view and keeps the sorted rows per revision.
- HPR-04: Pages list lays out only the rows in view.

Next:
- UWL-05 second half: the Learn window (resizable `egui::Window`, kept id) stays a sliver after the window was shrunk and grown back. Not fixed.
- HPR-04 per-edit cost (delete of one page 1.3 s at 1000 pages): whole-project rebuild per revision, needs a profile.
- Page filter at 1000 pages (`net_sections` / `wire_groups` scan wires per page).
