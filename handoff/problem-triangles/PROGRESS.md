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

## 2026-11-04 Lane T11 (EMC, safety, protection, calculations, standards)
- `app/flags.rs`: `source_of(kind)` gives EMC findings `Source::Emc`, safety function findings `Source::Safety`, cable rating / short-circuit / disconnection / thermal / voltage drop / motor starter / protective conductor findings `Source::Calc`; the standards rules stay `Issues(check.standard)`. `sync_flags` replaces per engine. Waiver keys unchanged.
- Popover: a "?" (help mark) beside the badge holds the standing caveat of the source ("design aid, not compliance"; safety uses `novo_safety::DISCLAIMER`). EMC triangles have "Preview the fix" (`emc_flag_fix`: first applicable suggestion of that finding, shown in the EMC window as a preview, nothing changes until Apply) and all five sources have "Open <window>" (check.emc, check.safety, check.calculations, check.standards).
- `flag_marks`: `Anchor::Device` is drawn on the device's symbols on the page (before: device findings had no mark on the schematic). Terminal and Cable anchors still not drawn.
- Census: `census_safety_tests.rs`, ceiling `handoff/problem-triangles/ceilings/safety.txt` = 66 baseline (no sites removed yet).
- NOT done in this lane (next PR): the findings tables of the EMC window, Calculations and Safety as filtered views of the store; migrating the 66 coloured/banner/toast sites; Structure/Plate anchors (EMC plate findings); `S1/S2` of novo-safety are risk-graph classes, not severities, and are untouched.
