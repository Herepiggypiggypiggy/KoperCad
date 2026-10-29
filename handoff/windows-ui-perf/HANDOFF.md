# Windows UI and performance findings: handoff

Branch claude/project-thread-5dsxif (kopercad). PR: see PROGRESS.md.

Root cause worth knowing: `egui::Modal` sizes its area from its content including the full-screen backdrop, so after a shrink the area keeps the old size and the dialog sits off screen. Fix is central (appearance.rs), not per dialog. Test: `small_window_tests::preferences_stays_usable_after_the_window_shrinks`.

Measuring (release, software Vulkan, 1366x768, UI pass): `cargo build --release -p novo-app -p novo-cli`, generate files with
`target/release/examples/dense_page FILE 5000` (one page, HPR-05) and `... FILE 50000 1000` (1000 pages of 50 symbols, HPR-04), then `scripts/qa.sh run --slow my.qa` with `open FILE`, `frame_stats none 60`. Before/after: Commissioning on 5000 symbols 66 to 14 ms; Pages panel open at 1000 pages 35 ms (before) vs 13 ms with it closed.
HPR-04 as reported (60 ms idle) did not reproduce with the Pages panel closed: 13 ms.

QA scenario: qa/scenarios/1590-prefs-interface-200.qa.
Traps: Learn window sliver not fixed; sort cache in commissioning is keyed on revision plus machine generation.
