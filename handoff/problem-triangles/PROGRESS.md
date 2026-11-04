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

## 2026-11-04 Lane T5 (wiring flags)
- `app/flags_wiring.rs` (new): wire-end checks (WireRange, PointOccupied, FerruleUse) are anchored `Anchor::Pin(symbol, pin)` (found from the wire ends in `sync_flags`) and drawn as a mark on the pin point (`MarkTarget::Area`). Waiver keys unchanged. Terminals drawer: the same triangle leads the Accepts cell of the row.
- Job flags: `NovoApp::job_flag(job, red, title, detail, JobRetry)` / `job_clear(job)` (Source::Job, `Anchor::Job`, `Life::Until`). Used by Wire-it export, wire cut, wire production (xlsx, page, wiring plan DXF) and the workshop pack: failures red, "nothing to do" yellow, cleared when the same job succeeds. Top bar: `job_roll_up` beside the Issues pill; Wire-it strip: triangle on Print list. Popover gets Try again (`FlagUi.retry`) and Dismiss (any `Life::Until` flag). T12 can reuse `job_flag` with its own `JobRetry` variants.
- Wire panel: a refused edit (`WireEdit::field()`) is a flag `Anchor::Control("wire_panel.<field>")` with a triangle beside the field; cleared when the field takes an edit, the wire changes, or none is selected.
- Census: `problem_census_wiring` (own file `app/problem_census_wiring_tests.rs`), ceiling 15 → 5. The 5 left are the Wire-it strip's coloured text (length basis, over-full fill, picture note), owned by T4 (`view3d_wire_it.rs`, `view3d_wires.rs` rows stay with T4).
- Not done: `view3d_wires.rs` rows (T4); Info refusals stay as status toasts (class N); terminals.rs other toasts (T8). QA 9450-9452; the wire panel field is covered by a unit test only (needs a wire click coordinate per size).
