# ui-multi-property-sheet handoff

Branch `ui-multi-property-sheet`, one PR from main. Code: `crates/novo-app/src/app/selection_sheet.rs` (UI and actions),
tests in `selection_sheet_tests.rs`; hook in `inspector.rs` (`multi_inspector` when no single device is selected);
command registered in novo-workflow `commands.rs`, `language.rs`, `menus.rs`, `tests/journeys.rs`.

Resume: `cargo test -p novo-app selection_sheet`, `cargo test -p novo-workflow`, `cargo test -p novo-parts --test parts`.

Traps: `edit.properties` already exists as an editor-queued id (focus the tag field), so the new command is `edit.shared_properties`.
The window and the Inspector share one state (`wb.selection_sheet`); the window stays shut for two or more devices to avoid duplicate widget ids.
