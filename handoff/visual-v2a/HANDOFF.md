# Visual redesign lane V2a (top bar): handoff

## What exists
- `crates/novo-app/src/app/top_bar.rs`: `undo_redo_buttons`, `page_switcher_button` / `page_switcher_ui`, `PageSwitcher` state in `Workbench::page_switcher`.
- `workspace.rs`: `workspace_switcher(ui, SegmentNames)` paints the five workspaces as one track; `Workspace::glyph()` names each icon. `menu_bar.rs::TopBarFit::segment_names` chooses by width: all names from 1500, the current one from 760, icons only below.
- `app.rs`: `TOP_BAR = 40`. Undo/redo show from 1250 points, the page caret from 1250 (narrower, the menu words need the room; the breadcrumb page name still opens the outline).
- Lesson anchor `page.switcher` (registered in `learn.rs` and `learn_labels.rs`, doc regenerated).
- Tests: `top_bar_tests.rs`, the existing width tests (no overlap at 1100 to 1920). QA scenario `qa/scenarios/722-top-bar.qa`.

## Rules for the next lanes
- Undo and redo are still in the dock as well until V3 removes them there.
- V2b owns the app menu (`app_menu.rs`); the Help button fold waits for it.

## Not verified
Windows fonts, 125 and 150 percent scaling, a real screen reader.
