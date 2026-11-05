# Parts and Macros help sweep (PR 1975)

Parts and Macros (second PR, 2026-11-03)
- Done: Parts window and its sub-forms (parts.rs, parts_bundle, parts_cad, parts_quality, parts_table, parts_bulk, eplan_parts), Place macro (macro_actions), Macros window (macro_browser), macro editor, mounting view. Macro descriptions and parameter descriptions are behind the "?" of the Place macro window; the list rows show the name only (description on hover).
- `panel::modal` floating windows are now wrapped in `window_help::scope` (name = dock name), so any dockable form shows a "?" in its title bar. Questions in `MODAL_QUESTIONS` keep their text (no header "?").
- Left: hover texts (hidden), error toasts, result sentences over 20 words (import summaries, Takes -F1... tag lists), maker_guide lines (data), pin_offer question.
- Scenarios changed: 702, 706, 1583, 17100, 17100b, 21 (strings). New: 7432.
2026-11-03 18:10 UTC Parts and Macros sweep (branch claude/project-thread-ss17xr): Parts window (maker guide, ETIM install, empty list, import wait/drop texts, source, 3D model status, detail), library export/import, CAD files, quality review, table import, EPLAN/maker XML import, auto-assign, Place macro, Macros window, macro editor, mounting view. Floating forms (panel::modal) now run inside window_help::scope, so they get a "?" in the title bar; modal questions (MODAL_QUESTIONS) keep their text. Scenario 7432.

Also done in this PR: placement toasts and refusals keep their full wording (toasts have no ?); the mounting view line carries a visible (suggestion) tag with the full caveat behind the ?.
Follow-ups: scenario 702 expects 'Keep it for yourself', which main hides behind the backup triangle; 7432 check_text flags catalog descriptions at 1920; the Place macro dock clips at the Place button.
