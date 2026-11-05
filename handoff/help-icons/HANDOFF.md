# Help icons handoff (2026-11-03)

Branch `claude/project-thread-rzc31f`. First PR: the "?" mechanism plus 3D Heat, Vents, Hoist run, Hoist in 3D, Drive run.

Done in the first PR
- `window_help.rs`: `scope` (wraps a window's content, set by `Panel::show`, `Dialog::show`, `show_docked`), `add(ctx, text)`, `mark(ui, text)`, header "?" for docked panes (`panel_dock::show_docked`) and floating windows (`panel_dock::title_buttons`). Modals that float have no "?" yet.
- Moved behind "?": Heat strip layer notes (`heat_legend::help`), Heat panel status/phase/"What is this?", verdict caveat, Vents and fans window texts, Cooling selection notes, Cooling what-if banner text, Hoist run, Hoist in 3D, Drive run (intro, model notes, duty cycle intro, plot legend).
- `check_text [max_words]` QA step (S3 finding) and `KOPERCAD_QA_TEXT_AUDIT=<file>` runtime audit; scenarios 7430 and 7431.

Not done (see INVENTORY.md for every long label found)
- A `window_help::add` with no window being built is dropped: Cabinet 2D legend, Cabinet strip, status bar, start screen, canvas overlays must use `window_help::mark` or get a header.
- Long result sentences stay (Cooling selection proposes ..., report summaries): shorten them at the source.
- Hover tooltips were left alone (hidden until pointed at).
- Floating modal forms and the add-on / Learn cards have no header "?".
- Status bar hints and the X/Y readout (40 scenarios).

Resume: `scripts/qa.sh --only help-icons --out <dir>` (needs `KOPERCAD_BIN_DIR` after `cargo build -p novo-app -p novo-cli`). To re-measure a whole area: set `KOPERCAD_QA_TEXT_AUDIT`.
Traps: a lesson or scenario that quotes removed text (grep `assets/lessons` and `qa/scenarios` for the old wording; heat-and-sizing quotes the 2D legend, which still shows it).

Parts and Macros (second PR, 2026-11-03)
- Done: Parts window and its sub-forms (parts.rs, parts_bundle, parts_cad, parts_quality, parts_table, parts_bulk, eplan_parts), Place macro (macro_actions), Macros window (macro_browser), macro editor, mounting view. Macro descriptions and parameter descriptions are behind the "?" of the Place macro window; the list rows show the name only (description on hover).
- `panel::modal` floating windows are now wrapped in `window_help::scope` (name = dock name), so any dockable form shows a "?" in its title bar. Questions in `MODAL_QUESTIONS` keep their text (no header "?").
- Left: hover texts (hidden), error toasts, result sentences over 20 words (import summaries, Takes -F1... tag lists), maker_guide lines (data), pin_offer question.
- Scenarios changed: 702, 706, 1583, 17100, 17100b, 21 (strings). New: 7432.

Slice 2 (this PR): `window_help::add` where the surface is a panel or dialog, `window_help::mark` for headerless rows. Still long: report documents themselves (EMC record, Contents.pdf are files, not screens), Commissioning hint sentences from `step.same_number_hint()` (now behind "?"), hover tooltips, `emc_stub.rs` finding details (result text), Reports preview pages, Bench "Not set:" banner (result sentence, 29 words).
