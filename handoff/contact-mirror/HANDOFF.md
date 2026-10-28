# Contact mirror handoff

- Branch `claude/project-thread-kgxn9l`, PR from main (see PROGRESS.md for the number and head).
- Layout lives in `crates/novo-canvas/src/note_table.rs` (pure: rows in, primitives and labels out; canvas and PDF share it). `NoteRow::poles()` splits `13-14` / `1-2 3-4 5-6` / `11-12-14` into terminals; designations that do not split fall back to the old text form right of the symbol.
- `PageScene::note_row_at(world)` finds the coil and contact under a point; `SchematicView::take_note_jump()` hands a double click to the app (`app/xref.rs::follow_note_jump`).
- Spare rows: `XrefIndex::label(..).spare`; the app adds them after the drawn rows unless `Settings::xref_hide_spare`. At most `MAX_SPARE_ROWS` (32) are listed, then one line `and N more spare` (`Label::spare_more_line`), so a mistyped `contacts.no` stays a short table. **Spare numbers are a guess**: they continue the EN 50005 sequence of the drawn contacts (43-44, 53-54, ...) and are not read from the part. They are drawn dim and printed grey; a part-data lane should replace them with the maker's numbers. They are left empty when a drawn contact does not follow EN 50005.
- Fixture `qa/fixtures/contact-mirror.kopercad` from `cargo run -p novo-tests --example contact_mirror_fixture -- qa/fixtures`; QA scenarios `qa/scenarios/642-contact-mirror.qa` (1366x768) and `643-contact-mirror-1920.qa` (1920x1080), new harness step `double_click X Y`. At 1920 a first click selects the coil and the Inspector narrows the canvas, so the page refits: click once, then double click at the refitted position.
- App tests: `app/xref_table_tests.rs`.
- The mapping from `Label` to `SymbolNote` exists twice (`app/xref.rs::note`, `novo-tests/src/text_conflicts.rs::note_of`); change both.
- A jump from a double click is made after the canvas finished its frame (`Workbench::deferred_jump`); jumping inside the canvas pass selected the target on the old page's scene.
- Pictures from this work: /mnt/project-files/qa/contact-mirror/ (screens, PDF of the fixture).
