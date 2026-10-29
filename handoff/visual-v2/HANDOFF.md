# Visual redesign lane V2: handoff

## What exists
- `crates/novo-app/src/app/app_menu.rs`: `mark()` (the button, a caret, `Popup::menu`), `app_menu_ui` (search, area list, rows, footer), `app_menu_keys` (arrows, Enter, Alt+letter), `area_entries` and `search_entries` (the data, tested without a window), `AppMenu` state in `Workbench::app_menu`.
- `Settings::menu_names: Option<bool>`, `show_menu_names()`, `MENU_NAMES_DEFAULT` (true for now). `NovoApp::menu_names_shown()` is wide window and setting; the top bar draws the menu words only then.
- `menu_bar.rs::hint_for` (tooltip sentences of export commands) is shared by the bar rows and the app menu rows.
- Tests: `app_menu_tests.rs` (every bar command is in the app menu, groups, search, F10 with names hidden and shown, Enter, Alt+letter, arrows, both themes at both sizes). QA scenario `qa/scenarios/721-app-menu.qa`.

## How a row looks
Icon (drawn or Phosphor, `presentation::glyph`), short label (`presentation::short_label`), key cap right; the tooltip is the full label, a hint sentence, and the reason when the row is grey. Submenus are folded groups with their count; a search shows every match with its menu path.

## Rules for the next lanes
- A new command needs a presentation row (V0) and appears in the app menu by itself (it reads `menus::drawn`).
- Lesson anchors: with the menu words hidden, `menu.<Area>` anchors sit on the area rows of the open app menu; `menu.project` stays on the mark. With the words shown nothing changed.
- V2c trims the registry; the app menu follows without a code change.
- V3 (rail, other thread) and V5 (palettes, other thread) do not touch `app_menu.rs`.

## Not verified
Windows fonts, 125 and 150 percent scaling, real GPU colour; a real screen reader (the painted rows carry a `WidgetInfo` label, so the accessibility tree and the QA `click_text` see them, but no screen reader was run).
