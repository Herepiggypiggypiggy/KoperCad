2026-10-28 11:00 UTC P1 engine and app panel written; tests run in progress.
2026-10-28 P1 done: clippy clean, novo-parts and novo-app tests pass (1663 + 80 + engine tests), QA scenario 700 passes at both sizes (/mnt/project-files/qa/parts-cad-match/). PR opened from claude/parts-cad-match.
Next: P2 (BMEcat 1.2, prices, markup text, ETIM from a user-supplied release, KC-09 checks), P3 (library export/import). After PR 1563 merges: switch identity_of to measure_step.
2026-10-28 12:20 UTC After 1563 merged: rebased #1572 on main, identity_of measures the solid; real LC1D09 gives 45 x 77 x 95 mm, size fits.
2026-10-28 P2 (#1578, BMEcat 1.2 prices ETIM) and #1598 (CAD files read on a worker thread) in review. P3 written on claude/parts-library-bundle from main: `novo-parts::bundle`, CLI `parts export-library` / `import-library`, Parts window More > Export library / Import library, `.kcpl` drop. Tests and clippy pass; QA scenario 702.
