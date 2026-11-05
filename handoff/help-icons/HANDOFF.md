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

Second PR (Learn, Assistant, Start, Help, App menu, Preferences): see PROGRESS.md 18:10 entry. Still open: Parts, Macros, Cabinet, Reports, Schematic, 3D, Check, Add-ons (see INVENTORY.md). Learn lesson-step text and the Ask Claude consent notice stay visible by design.
Slice 2 (this PR): `window_help::add` where the surface is a panel or dialog, `window_help::mark` for headerless rows. Still long: report documents themselves (EMC record, Contents.pdf are files, not screens), Commissioning hint sentences from `step.same_number_hint()` (now behind "?"), hover tooltips, `emc_stub.rs` finding details (result text), Reports preview pages, Bench "Not set:" banner (result sentence, 29 words).

## Cabinet area (done 2026-11-03)
Files: cabinet.rs, cabinet_routes.rs, cabinet_door.rs, cabinet_panel.rs, cabinet_rails.rs, cabinet_rules.rs, cabinet_rules_panel.rs, cabinet_surface_size.rs. Inside a Panel or dialog the text goes through `window_help::add`; in raw egui side panels (Routes, Unplaced) and banners it is `window_help::mark`. Left: EMC check window texts in emc_view.rs, per-surface Info notes (`surface_notes` in cabinet.rs).
