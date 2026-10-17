# Revisions follow-ups: handoff

State: nothing in flight. All four items are merged or in a READY PR.

- Outline Changes list: `changes_outline` and `page_has_changes` in `crates/novo-app/src/app/changes.rs`.
- PDF: `PdfOptions.title_revisions` and `changed_pages` (novo-pdf); the app fills them from `novo_revisions`.
- Revision table: `TitleRevisions::table` in novo-canvas, drawn by `PageScene::revision_table`; test in `crates/novo-pdf/tests/revision_table.rs`.
- Card close: `WorkbenchState.changes_closed` is set when the Changes card is closed; the Changes tab then shows "Changes mode is off" with a "Turn on" button until another outline tab is shown.

Gotcha: a PR's CI runs on the merge with main at the time; a failure from a broken main needs a main merge push, not a re-run.
