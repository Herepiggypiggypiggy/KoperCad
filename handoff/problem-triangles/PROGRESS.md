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

## 2026-11-04 Lane T7 (schematic edit refusals)
- `app/flags_edit.rs`: `refuse` / `refuse_titled` / `refuse_tool` replace the warning and error toasts of refused edits. One transient flag (`Source::Edit`, badge "Edit", `Life::Until`) on the selected symbols and wires (at most 3), else on the tool in hand (`Anchor::Control("tool")`, drawn at the top left of the canvas). It goes with the next key or click (counted on release, so a click on its triangle opens the popover first). The text stays in the message log, no toast. Transient flags are not Issues: the pill and F8 do not count them.
- The long-refusal card of the status bar is gone: the tool's own refusal (editor notice) is the same corner triangle and goes with the notice. The bar still shows the short hint.
- Anchors the schematic ignored are drawn now: `Page` (a spot on the sheet), `Device` (its first symbol on the page), `Terminal` (its first symbol), `Cable` (its first wire), `Pin` (its symbol). Kinds of the cabinet, routing and EMC lanes keep their device anchors for the plate and 3D (`is_physical`). One triangle per object per flag.
- Migrated files (toast Level::Warning/Error sites: 0 left; census `problem_census_schematic_edits`, ceiling 0): inspector, clipboard, duplicate, pages, outline, macro_actions, macro_browser, macro_editor, tidy, pin_offer, potentials. Class J rows ("Could not change it: ...") use the same transient triangle, red.
- Not touched: banners and in-window notices inside the macro dialogs and macro editor (class P inside a form, left for the lane that owns the dialog), numbering.rs and link.rs (T8), class N info refusals (stay in the status bar), class T text (text sweep thread).
- QA: 9470 (1366x768), 9471 (1920x1080): bad tag on the stop button, triangle, popover, Esc, next click removes it.
## 2026-11-04 Lane T12 (import, export, reports): job flags
- `app/flags_jobs.rs`: `job_fail`/`job_fail_at`/`job_ok`, `job_pill` (top bar triangle with count), popover actions Retry / Open folder / Copy details / Dismiss for `Source::Job` flags. 48 failed-job toasts in the files area now use it; census `problem_census_files` 66 to 18 (`app/problem_census_files.txt`). Lane folder `t12-files/`. QA 9522, 9523.

- T12 PR 2 (bench): `widgets::flag_chip` for inline problems on a section (triangle, short title, popover with the sentence); bench banners and readiness gaps migrated; census files 18 to 13. QA 9524/9525.
- T12 PR 3: plot sets, report templates and bench packet refusals are job flags; census files 13 to 8.
- T12 PR 4: DWG converter, handover, revision, change report, designer and manual refusals are job flags; files area down to the one written-with-warnings toast.
