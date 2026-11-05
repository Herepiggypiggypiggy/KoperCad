# T1 handoff

Branch `claude/project-thread-h0lpex` (kopercad). One PR from main, title "Problem triangles T1: ...". State: see PROGRESS.md for the head and review state.

Resume: `cargo test -p novo-app --lib cabinet_flags` (new tests), `scripts/qa.sh --only 9410`.

How it works
- Issues problems -> `flags::drafts` -> `wb.flags.store` (Source::Issues). Structure ids in `Problem::at` now become `Anchor::Plate(structure, [0,0])`.
- Cabinet-only flags (Place rules, wiring openings) are `Source::Layout`, rebuilt by `flags::sync_layout_flags` when the project revision or the rules change. They have no Issues index: Locate selects the device on its plate (`locate_on_plate`), no Waive.
- `cabinet_show` calls `shown_problems()` + `sync_flags()` first, so the plate shows the latest checks. Triangles lag a drag by one check (they follow the stored layout, the box outline follows the drag).

Traps
- Two identical problems share one flag id: Layout rule keys include the devices and the side.
- Unplaced devices are Info in novo-checks, so they have no triangle (the "N unplaced" pill stays).
- The old hover line "Size is a guess" and the "No problems" line are gone on purpose (all-clear sentence); the dashed `~` size on the box stays.
- Ceiling file: `app/census/cabinet.ceiling`, test `problem_census_cabinet`.

Follow-up #1954: Layout flags are counted and listed. Open gap: a QA fixture that shows a rule flag row in the Issues list.
