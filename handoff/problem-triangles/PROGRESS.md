2026-11-04 08:30 UTC audit scanned (76 kinds, 50 finding types, 776 sites); LEDGER.md and sites/*.md written
2026-11-04 08:50 UTC design doc written (docs/design/problem-triangles.md), 16 lanes planned

## 2026-11-04 Lane T0a (novo-flags model and store)
- `novo-flags` now holds `Flag`, `FlagDraft`, `Anchor` (string ids, so no novo-core dependency), `Severity`, `map_word`, `FlagId` (FNV-1a of source, rule, sorted anchors) and `FlagStore` (`replace_source`, `set`/`clear`, waivers by id and by Issues key, `take_fresh` for the pulse, `at`, `roll_up`, `roll_up_where`, `next` for F8, `by_area`, `counts`). 12 tests.
- Adapters in novo-app (lane T0b) build `FlagDraft`s; drawing, popover, Flags list and top bar pill are T0b, not here.

## 2026-11-04 Lane T0b (schematic migrated)
- `novo-canvas`: `MarkKind::Warning` (yellow hollow), `Error` (red solid, the octagon is gone), new `Unchecked` (dashed yellow); glyph 14 pt; `Mark.key` carries the flag id.
- `novo-editor`: `EditorView::take_mark_click()` hands back the keys of the marks under a primary click.
- `novo-theme`: `widgets::flag_triangle` (painted glyph), Issues pill draws triangles.
- `novo-app` `app/flags.rs`: `draft(&Problem)` -> `FlagDraft` for all 76 kinds (Info is no flag; DepthUnknown, GuessedWire, UnsizedDuct dashed), `FlagUi` in `wb.flags` filled from `wb.problems` by revision, `flag_marks` (replaces the old marks), `flag_popover` (title, badge, detail, Locate / Fix / Waive / Explain / Copy; Esc or click away closes), Issues list rows lead with the triangle. Qa scenario 9400.
- Not in this lane: Flags list grouping/filters by source, Anchor::Device/Terminal/Cable drawn on the schematic (still no drawn object), cabinet/3D surfaces (T1, T3), toasts (T7+).

## 2026-11-04 Lane T8 (terminals, cables, PLC, numbering, xref)
- `app/flags_inline.rs`: `triangle` (20 pt slot, accessible name "Warning: title", click opens the detail), `note_row`/`flag_row` (triangle + plain title), `severity_of` (numbering words go through `novo_flags::map_word`, so its Info<Warning<Error order cannot flip), and the transient refusal: `NovoApp::refuse` / `refuse_red` show a chip at the bottom right of the canvas (not a toast, not in the count); the next key press or click elsewhere, or 12 s, clears it.
- Cable drawer: the findings are the store's flags at `Anchor::Cable` (same flags as Check > Issues), one triangle row each, sentence in the detail. Terminals drawer: a triangle after the tick box on rows whose terminals or drawing symbols carry a `TerminalUse` flag. Rename terminals: taken/empty rows are red triangles. PLC overview, card dialog and PLC program window: triangles instead of coloured sentences. Wire numbering Conflicts tab: triangle per row (glyph characters gone).
- Toasts to refusal chips: terminal edit (6), cable tag/type (5), card layout (1). 14 P-class toasts converted; 13 coloured sentences (8 sites) gone.
- Census: `app/problem_census_terminals.rs`, ceiling 33 toasts (was 46) and 0 coloured sentences (was 8), own file.
- Left on purpose: Job toasts ("Could not ...: {e}", 22 in this area) wait for the job chip (T12/T15). The wire-number refusals in `numbering.rs` stay toasts because the wire-numbers lesson quotes them. Xref has no problem sentence of its own: its four index issues already reach the symbol as flags through `ProblemKind`.
- QA: 9480 (numbering conflicts), 9481 (refusal chip).
