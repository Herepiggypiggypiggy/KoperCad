# Drill and cut-out templates: handoff

Branch `claude/project-thread-vgkwt9` (kopercad), one PR from main (see PROGRESS.md for number and head).

Code: `crates/novo-layout/src/cutout_cards.rs`, `crates/novo-pdf/src/template_cards.rs` (+ `tests/template_cards.rs`), `crates/novo-app/src/app/workshop.rs` (`export_scale_check`, pack files), `bench.rs` (`print_check` row), command `export.scale_check` (commands.rs, menus.rs, language.rs, workbench.rs, menu_bar.rs), QA `qa/scenarios/39-print-scale-check.qa`.

Decisions
- Scale check uses a square (both axes) plus a 250 mm ruler (feed over a door length); tolerance 0.2 mm per 100 mm, a difference over 0.3 mm between sides is "stretched" (no percentage helps).
- Cards group by cut-out shape per surface; positions are the drill table's x and y plus the distances from all four edges. Cut-outs over 150 mm are named as left out, never dropped silently.
- Honesty: printed accuracy depends on the printer; about 0.5 mm is an estimate, not measured.
- novo-core untouched.

Resume: `cargo test -p novo-layout --lib cutout_cards`, `cargo test -p novo-pdf --test template_cards`, `cargo test -p novo-app --lib workshop`, `scripts/qa.sh --only print-scale`.
Evidence: /mnt/project-files/qa/drill-templates/ (shots 1366x768 and 1920x1080, sample PDFs).
