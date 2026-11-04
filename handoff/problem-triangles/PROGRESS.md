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

## 2026-11-04 Lane T9 (parts and catalogue)
- New `app/parts_flags.rs`: `RowFlag` + `slot`/`inline`/`labelled` (a 20 pt slot with the triangle and a click popover, accessible name "Error: <title>"), and job flags: `parts_job_failed(job, severity, title, detail)` sets a `Source::Parts` flag on `Anchor::Job("parts:<job>")` (`Life::Until`), `parts_job_ok(job)` clears it, `job_strip` draws them beside More in the Parts window with Dismiss and Copy. While the Parts window is closed the failure is still said once as a toast (bridge until the job tray, T12/T14).
- Migrated: table import preview (leading triangle per row with column/message lines, columns problems, assumptions, repeated order numbers, footer), EPLAN parts preview, bulk import and auto-assign (not certain, no part fits, files without parts), part picker (weak match, thin data), trust badge and review (quality), pins read failure, library bundle caution, download strip (not taken), import failed. Toasts at `Level::Error/Warning` in these files became job flags (jobs: table, eplan, eplan.makers, import, inbox, catalog, assign, remove, set, window, folder).
- Census `crates/novo-app/tests/problem_census_parts.rs` with ceilings in `tests/census/parts.txt`: 92 sites before, 22 after (quality grade/fill colours are a scale and stay; the picker match colour helper is tested; one toast bridge).
- Not done: the Flags list and the top bar pill do not yet count or list `Source::Parts` job flags (store holds them); `Note` rows (novo-parts Severity::Note) never become flags and are only read in the popover of a row that has a real finding; EPLAN/portal sentences beyond the ones above.
- QA: 9490 row triangle and popover, 9491 job flag and Dismiss, 9492 columns mapping.
